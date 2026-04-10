// SPDX-License-Identifier: GPL-2.0-only

/// Create a Linux machine and run it.
const linux = async (worker_url, vmlinux, boot_cmdline, initrd, log, console_write) => {
  /// Dict of online CPUs.
  const cpus = {};

  /// Dict of tasks.
  const tasks = {};

  /// Set of online CPU IDs (for heartbeat).
  const onlineCpus = new Set();

  /// Timer heartbeat: periodically raises WASM_IRQ_TIMER on non-IRQ CPUs
  /// to work around broken timer delegation on hotplugged nohz_full CPUs.
  const WASM_IRQ_TIMER = 1;
  const IRQ_CPU = 1;
  let heartbeatStarted = false;

  /// Input buffer (from keyboard to tty).
  let input_buffer = new ArrayBuffer(0);

  /// Pending console_read request (deferred until input arrives).
  let pendingConsoleRead = null;

  const text_decoder = new TextDecoder("utf-8");
  const text_encoder = new TextEncoder();

  const lock_notify = (locks, lock, count) => {
    Atomics.store(locks._memory, locks[lock], 1);
    Atomics.notify(locks._memory, locks[lock], count || 1);
  };

  const lock_wait = (locks, lock) => {
    Atomics.wait(locks._memory, locks[lock], 0);
    Atomics.store(locks._memory, locks[lock], 0);
  };

  /// Callbacks from Web Workers (each one representing one task).
  const message_callbacks = {
    start_primary: (message) => {
      // CPU 0 has init_task which sits in static storage. After booting it becomes CPU 0's idle task. The runner will
      // in this special case tell us where it is so that we can register it.
      log("Starting cpu 0 with init_task " + message.init_task)
      onlineCpus.add(0);
      tasks[message.init_task] = cpus[0];
    },

    start_secondary: (message) => {
      if (message.cpu <= 0) {
        throw new Error("Trying to start secondary cpu with ID <= 0");
      }

      log("Starting cpu " + message.cpu + " (" + message.idle_task + ")" +
        " with start stack " + message.start_stack);
      onlineCpus.add(message.cpu);
      make_cpu(message.cpu, message.idle_task, message.start_stack);
      startHeartbeat();
    },

    stop_secondary: (message) => {
      if (message.cpu <= 0) {
        // If you arrive here, you probably got panic():ed with a broken stack.
        if (!confirm("Trying to stop secondary cpu with ID 0.\n\n" +
          "You probably got panic():ed with a broken stack. Continue?\n\n" +
          " (Say ok if you know what you are doing and want to catch the panic, otherwise cancel.)")) {
          throw new Error("Trying to stop secondary cpu with ID 0");
        }
      }

      onlineCpus.delete(message.cpu);

      if (cpus[message.cpu]) {
        log("[Main]: Stopping CPU " + message.cpu);
        cpus[message.cpu].worker.terminate();
        delete cpus[message.cpu];
      } else {
        log("[Main]: Tried to stop CPU " + message.cpu + " but it was already stopped (broken system)!");
      }
    },

    create_and_run_task: (message) => {
      // ret_from_fork will make sure the task switch finishes.
      make_task(message.prev_task, message.new_task, message.name, message.user_executable);
    },

    release_task: (message) => {
      // Stop the worker, which will stop script execution. This is safe as the task should be hanging on a lock waiting
      // to be scheduled - which never happens as dead tasks don't get ever get scheduled.
      tasks[message.dead_task].worker.terminate();

      delete tasks[message.dead_task];
    },

    serialize_tasks: (message) => {
      // next_task was previously suspended, wake it up.

      // Tell the next task where we switched from, so that it can finish the task switch.
      tasks[message.next_task].last_task[0] = message.prev_task;

      // Release the above write of last_task and wake up the task.
      lock_notify(tasks[message.next_task].locks, "serialize");
    },

    console_read: (message, worker) => {
      const buffer = new Uint8Array(input_buffer);
      if (buffer.length > 0) {
        // Input available — deliver immediately.
        const memory_u8 = new Uint8Array(memory.buffer);
        const used = buffer.slice(0, message.count);
        memory_u8.set(used, message.buffer);
        const unused = buffer.slice(message.count);
        input_buffer = unused.buffer;
        Atomics.store(message.console_read_messenger, 0, used.length);
        Atomics.notify(message.console_read_messenger, 0, 1);
      } else {
        // No input yet — defer. Worker waits up to 50ms for key_input
        // to arrive and fulfill this request via fulfillPendingConsoleRead.
        pendingConsoleRead = message;
      }
    },

    console_write: (message) => {
      console_write(message.message);
    },

    log: (message) => {
      log(message.message);
    },

    // Network I/O handlers (bridge worker ↔ NetProxy on main thread).

    net_send: (message) => {
      if (typeof NetProxy !== 'undefined') {
        NetProxy.send(new Uint8Array(message.packet));
      }
    },

    net_recv: (message) => {
      const memory_u8 = new Uint8Array(memory.buffer);
      let count = 0;
      if (typeof NetProxy !== 'undefined') {
        const packet = NetProxy.recv(message.max_length);
        if (packet && packet.length > 0) {
          memory_u8.set(packet, message.buffer);
          count = packet.length;
        }
      }
      Atomics.store(message.net_recv_messenger, 0, count);
      Atomics.notify(message.net_recv_messenger, 0, 1);
    },

    net_poll: (message) => {
      let count = 0;
      if (typeof NetProxy !== 'undefined') {
        count = NetProxy.poll();
      }
      Atomics.store(message.net_poll_messenger, 0, count);
      Atomics.notify(message.net_poll_messenger, 0, 1);
    },
  };

  /// Memory shared between all CPUs.
  const memory = new WebAssembly.Memory({
    initial: 30, // TODO: extract this automatically from vmlinux.
    maximum: 0x10000, // Allow the full 32-bit address space to be allocated.
    shared: true,
  });

  /**
   * Create and run one CPU in a background thread (a Web Worker).
   *
   * This will run boot code for the CPU, and then drop to run the idle task. For CPU 0 this involves booting the entire
   * system, including bringing up secondary CPUs at the end, while for secondary CPUs, this just means some
   * book-keeping before dropping into their own idle tasks.
   */
  const make_cpu = (cpu, idle_task, start_stack) => {
    const options = {
      runner_type: (cpu == 0) ? "primary_cpu" : "secondary_cpu",
      start_stack: start_stack,  // undefined for CPU 0
    };

    if (cpu == 0) {
      options.boot_cmdline = boot_cmdline;
      options.initrd = initrd;
      initrd = null;  // allow gc
    }

    // idle_task is undefined for cpu 0, we will know it first when start_primary notifies us.
    const name = "CPU " + cpu + " [boot+idle]" + (cpu != 0 ? " (" + idle_task + ")" : "");

    const runner = make_vmlinux_runner(name, options);
    cpus[cpu] = runner;
    if (cpu != 0) {
      tasks[idle_task] = runner; // For CPU 0, start_primary does this registration for us.
    }
  };

  /**
   * Create and run one task. This task has been switch_to():ed by the scheduler for the first time.
   *
   * In the beginning, all tasks are serialized and have to cooperate to schedule eachother, but after secondary CPUs
   * are brought up, they can run concurrently (and will effectively be managed by the Wasm host OS). While we are not
   * able to suspend them from JS, the host OS will do that.
   */
  const make_task = (prev_task, new_task, name, user_executable) => {
    const options = {
      runner_type: "task",
      prev_task: prev_task,
      new_task: new_task,
      user_executable: user_executable,
    };
    tasks[new_task] = make_vmlinux_runner(name + " (" + new_task + ")", options);
  };

  /// Create a runner for vmlinux. It will run in a Web Worker and execute some specified code.
  const make_vmlinux_runner = (name, options) => {
    // Note: SharedWorker does not seem to allow WebAssembly Module or Memory instances posted.
    const worker = new Worker(worker_url, { name: name });

    let locks = {
      serialize: 0,
    };
    locks._memory = new Int32Array(new SharedArrayBuffer(Object.keys(locks).length * 4));

    // Store for last task when wasm_serialize() returns in switch_to(). Needed for each task, both normal ones and each
    // CPUs idle tasks (first called init_task (PID 0), not to be confused with init (PID 1) which is a normal task).
    const last_task = new Uint32Array(new SharedArrayBuffer(4));

    worker.onerror = (error) => {
      throw error;
    };

    worker.onmessage = (message_event) => {
      const data = message_event.data;
      message_callbacks[data.method](data, worker);
    };

    worker.onmessageerror = (error) => {
      throw error;
    };

    worker.postMessage({
      ...options,
      method: "init",
      vmlinux: vmlinux,
      memory: memory,
      locks: locks,
      last_task: last_task,
      runner_name: name,
    });

    return {
      worker: worker,
      locks: locks,
      last_task: last_task,
    };
  };

  // Create the primary cpu, it will later on callback to us and we start secondaries.
  make_cpu(0);

  /// Start the timer heartbeat: instantiate a lightweight vmlinux instance on
  /// the main thread and periodically call raise_interrupt(cpu, WASM_IRQ_TIMER)
  /// for each online non-IRQ CPU. This works around broken nohz_full timer
  /// delegation on hotplugged CPUs where arch_cpu_idle waits forever because
  /// local_timer_expiries is never programmed.
  ///
  /// raise_interrupt is pure WASM (atomic OR + notify on shared memory) and is
  /// safe to call from any context. The idle loop handles spurious wakeups
  /// gracefully: it re-checks expire times and only fires actual pending timers.
  const startHeartbeat = async () => {
    if (heartbeatStarted) return;
    heartbeatStarted = true;
    try {
      // Build stub imports — raise_interrupt uses NO host callbacks, but
      // WebAssembly.instantiate requires all declared imports to be present.
      const stub_imports = { env: { memory: memory } };
      for (const imp of WebAssembly.Module.imports(vmlinux)) {
        if (imp.module !== "env" || imp.name === "memory") continue;
        if (imp.kind === "function") {
          stub_imports.env[imp.name] =
            imp.name === "wasm_cpu_clock_get_monotonic" ? () => 0n : () => 0;
        } else if (imp.kind === "table") {
          stub_imports.env[imp.name] =
            new WebAssembly.Table({ initial: 4096, element: "anyfunc" });
        } else if (imp.kind === "global") {
          stub_imports.env[imp.name] =
            new WebAssembly.Global({ value: "i32", mutable: true }, 0);
        }
      }
      const hb = await WebAssembly.instantiate(vmlinux, stub_imports);
      const raise = hb.exports.raise_interrupt;
      if (!raise) {
        log("[Heartbeat] raise_interrupt not exported — disabled");
        return;
      }
      log("[Heartbeat] Timer assist active (200ms interval)");
      setInterval(() => {
        for (const cpu of onlineCpus) {
          if (cpu !== IRQ_CPU) {
            try { raise(cpu, WASM_IRQ_TIMER); } catch (_) {}
          }
        }
      }, 200);
    } catch (e) {
      log("[Heartbeat] Failed: " + e.message);
    }
  };

  /// Fulfill a pending console_read with whatever is in input_buffer.
  /// Uses CAS to safely race with the Worker's 50ms timeout.
  /// Messenger protocol: -1 = waiting, -2 = Worker left, -3 = main claiming, ≥0 = byte count.
  const fulfillPendingConsoleRead = () => {
    if (!pendingConsoleRead) return;
    const buf = new Uint8Array(input_buffer);
    if (buf.length === 0) return;
    const msg = pendingConsoleRead;
    pendingConsoleRead = null;

    // CAS from -1 (Worker waiting) to -3 (we're claiming).
    // If Worker already timed out (-2), this fails and data stays in input_buffer.
    const old = Atomics.compareExchange(msg.console_read_messenger, 0, -1, -3);
    if (old !== -1) return;

    // Worker is still waiting. Write data to WASM memory.
    const memory_u8 = new Uint8Array(memory.buffer);
    const used = buf.slice(0, msg.count);
    memory_u8.set(used, msg.buffer);
    const unused = buf.slice(msg.count);
    input_buffer = unused.buffer;

    // Signal the Worker with byte count and wake it.
    Atomics.store(msg.console_read_messenger, 0, used.length);
    Atomics.notify(msg.console_read_messenger, 0, 1);
  };

  return {
    key_input: (data) => {
      const key_buffer = text_encoder.encode(data);  // Possibly UTF-8 (up to 16 bits).

      // Append key_buffer to the end of input_buffer.
      // Use manual copy instead of ArrayBuffer.transfer() for broader compatibility.
      const old = new Uint8Array(input_buffer);
      const combined = new ArrayBuffer(old.byteLength + key_buffer.byteLength);
      const view = new Uint8Array(combined);
      view.set(old, 0);
      view.set(key_buffer, old.byteLength);
      input_buffer = combined;

      // Wake any blocked console_read immediately.
      fulfillPendingConsoleRead();
    }
  };
};
