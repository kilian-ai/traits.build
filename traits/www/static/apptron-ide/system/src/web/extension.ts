
import * as vscode from 'vscode';
import { WanixBridge } from './bridge.js';
// @ts-ignore
import monitorHtml from "./monitor.html";

const PTY_DEBUG_VERSION = "pty-bridge-selftest-20260421-04";

declare const navigator: unknown;

export async function activate(context: vscode.ExtensionContext) {
	if (typeof navigator !== 'object') {	// do not run under node.js
		console.error("not running in browser");
		return;
	}
	
	const channel = new MessageChannel();
	const bridge = new WanixBridge(channel.port2, "vm/1/fsys");
	context.subscriptions.push(bridge);

	const port = (context as any).messagePassingProtocol;
	port.postMessage({type: "_port", port: channel.port1}, [channel.port1]);

	bridge.ready.then((wfsys) => {
		console.log("bridge ready");
		const terminal = createTerminal(wfsys);
		context.subscriptions.push(terminal);
		terminal.show();

		context.subscriptions.push(vscode.commands.registerCommand('apptron.open-vm', (filepath?: string) => {
			const panel = vscode.window.createWebviewPanel(
				'apptron-vm',
				'Monitor',
				vscode.ViewColumn.One,
				{
					enableScripts: true
				}
			);
			panel.webview.html = monitorHtml;
			panel.webview.postMessage({ origin: location.origin, vm: filepath });
	
		}));
		vscode.window.registerWebviewPanelSerializer('apptron-vm', new WebViewPanelSerializer());

		(async () => {
			const dec = new TextDecoder();
			const stream = await wfsys.openReadable("#commands/data1");
			for await (const chunk of stream) {
				const args = dec.decode(chunk).trim().split(" ");
				const cmd = args.shift();
				vscode.commands.executeCommand(`apptron.${cmd}`, ...args);
			}
		})();		
	});


	context.subscriptions.push(vscode.commands.registerCommand('apptron.open-preview', (filepath?: string) => {
		if (!filepath) {
			return;
		}
		vscode.commands.executeCommand('markdown.showPreview', vscode.Uri.parse(`wanix://${filepath}`));
	}));

	context.subscriptions.push(vscode.commands.registerCommand('apptron.open-file', (filepath?: string) => {
		if (!filepath) {
			return;
		}
		vscode.commands.executeCommand('vscode.open', vscode.Uri.parse(`wanix://${filepath}`));
	}));

	context.subscriptions.push(vscode.commands.registerCommand('apptron.open-folder', (filepath?: string) => {
		if (!filepath) {
			return;
		}
		const folders = vscode.workspace.workspaceFolders;
		const insertIndex = folders ? folders.length : 0;
		const uri = vscode.Uri.parse(`wanix://${filepath}`);
		vscode.workspace.updateWorkspaceFolders(
			insertIndex, // insert at the end
			0, // number of folders to remove
			{ uri }
		);
	}));

	
	console.log('Apptron system extension activated');
}

function forceConsoleChannel(): boolean {
	try {
		const topSearch = window.top?.location?.search || "";
		const force = new URLSearchParams(topSearch || window.location.search).get("term_path");
		return force === "console";
	} catch {
		return false;
	}
}

async function tryAllocateXterm(wx: any): Promise<string | null> {
	try {
		const raw = await wx.readFile("web/dom/new/xterm");
		const terminalId = new TextDecoder().decode(raw).trim();
		if (!/^[0-9]+$/.test(terminalId)) {
			return null;
		}
		try {
			await wx.writeFile("task/1/ctl", new TextEncoder().encode(`bind #console/data web/dom/${terminalId}/data`));
		} catch {
			// Non-fatal: continue; shell may still expose the xterm path.
		}
		try {
			await wx.writeFile("web/dom/body/ctl", new TextEncoder().encode(`append-child ${terminalId}`));
		} catch {
			// Non-fatal in hidden/embedded contexts.
		}
		try {
			await wx.writeFile(`web/dom/${terminalId}/data`, new TextEncoder().encode("\n"));
		} catch {
			// Non-fatal: prompt can still appear after first keystroke.
		}
		try {
			await wx.writeFile("#console/data", new TextEncoder().encode("\r\n"));
		} catch {
			// Non-fatal: console channel may not be ready yet.
		}
		try {
			await wx.writeFile("vm/1/fsys/tmp/.apptron-terminal-id", new TextEncoder().encode(`${terminalId}\n`));
		} catch {
			// Best-effort only.
		}
		try {
			localStorage.setItem("apptron-terminal-id", terminalId);
		} catch {
			// localStorage may be unavailable in some extension host contexts.
		}
		// Return the bidirectional xterm pty channel, NOT #console/data.
		// #console/data is a one-way display pipe: writing to it renders text in
		// the xterm widget but does NOT feed the shell's stdin. The shell reads
		// stdin from web/dom/<id>/data (the xterm widget's data channel).
		// The bind above (task/1/ctl: #console/data -> web/dom/<id>/data) ensures
		// shell stdout flows into this channel too, making it fully bidirectional.
		return `web/dom/${terminalId}/data`;
	} catch {
		return null;
	}
}

async function tryFindExistingTerminalId(wx: any): Promise<string | null> {
	const idFiles = [
		"vm/1/fsys/tmp/.apptron-terminal-id",
		"/tmp/.apptron-terminal-id",
		"tmp/.apptron-terminal-id",
		"#console/terminalId",
	];
	for (const idFile of idFiles) {
		try {
			const raw = await wx.readFile(idFile);
			const terminalId = new TextDecoder().decode(raw).trim();
			if (/^[0-9]+$/.test(terminalId)) {
				return `web/dom/${terminalId}/data`;
			}
		} catch {
			// try next candidate
		}
	}
	try {
		const id = String(localStorage.getItem("apptron-terminal-id") || "").trim();
		if (/^[0-9]+$/.test(id)) {
			return `web/dom/${id}/data`;
		}
	} catch {
		// localStorage access can fail in restricted contexts
	}
	return null;
}

async function resolveTerminalDataPathOnce(wx: any): Promise<string | null> {
	if (forceConsoleChannel()) {
		return "#console/data";
	}
	// Prefer the hidden-shell bridge terminal when present so extension and
	// runtime attach to the same already-bootstrapped shell producer.
	const existing = await tryFindExistingTerminalId(wx);
	if (existing) {
		return existing;
	}
	// Fallback: allocate and fully wire a new xterm channel.
	const allocated = await tryAllocateXterm(wx);
	if (allocated) {
		return allocated;
	}
	return null;
}

async function resolveTerminalDataPathWithRetry(
	wx: any,
	writeEmitter: vscode.EventEmitter<string>,
): Promise<string> {
	// Retry schedule: probe aggressively at first, then back off. Total ~30s.
	const delaysMs = [
		0, 250, 500, 750, 1000, 1500, 2000, 2500, 3000,
		3000, 3000, 3000, 3000, 3000, 3000,
	];
	let announcedWaiting = false;
	for (let i = 0; i < delaysMs.length; i++) {
		if (delaysMs[i] > 0) {
			await new Promise((r) => setTimeout(r, delaysMs[i]));
		}
		const path = await resolveTerminalDataPathOnce(wx);
		if (path) {
			return path;
		}
		if (!announcedWaiting && i >= 2) {
			writeEmitter.fire("\r\n[apptron] waiting for Wanix terminal allocation...\r\n");
			announcedWaiting = true;
		}
	}
	writeEmitter.fire(
		"\r\n[apptron] terminal allocation failed after 30s. " +
		"Wanix runtime never provided a terminal id via web/dom/new/xterm or " +
		"vm/1/fsys/tmp/.apptron-terminal-id. Reload the page to retry, or " +
		"append ?term_path=console to force the legacy #console/data channel.\r\n",
	);
	throw new Error("terminal allocation timed out");
}

function createTerminal(wx: any) {
	const writeEmitter = new vscode.EventEmitter<string>();
	const dec = new TextDecoder();
	const enc = new TextEncoder();
	const debug = (msg: string) => {
		const line = `[apptron][pty] ${msg}`;
		console.log(line);
		writeEmitter.fire(`\r\n${line}\r\n`);
	};
	const emitBridgeSelfTest = async () => {
		let raw = "";
		try {
			const fromFile = await wx.readFile("vm/1/fsys/tmp/.apptron-bridge-selftest.json");
			raw = new TextDecoder().decode(fromFile).trim();
		} catch {
			// Fallback to localStorage when fs handoff isn't ready yet.
		}
		if (!raw) {
			try {
				raw = String(localStorage.getItem("apptron-bridge-selftest") || "").trim();
			} catch {
				// Ignore localStorage access failures.
			}
		}
		if (!raw) {
			debug("bridge self-test: no producer-side result found");
			return;
		}
		try {
			const parsed = JSON.parse(raw);
			debug(
				`bridge self-test: write_ok=${String(parsed?.write_ok)} token_seen=${String(parsed?.token_seen)} path=${String(parsed?.path || "?")}`,
			);
			if (Array.isArray(parsed?.notes) && parsed.notes.length > 0) {
				debug(`bridge self-test notes: ${parsed.notes.join(" | ")}`);
			}
		} catch {
			debug(`bridge self-test raw: ${raw.slice(0, 240)}`);
		}
	};
	const withTimeout = async <T>(promise: Promise<T>, ms: number, label: string): Promise<T> => {
		let timer: ReturnType<typeof setTimeout> | undefined;
		const timeout = new Promise<never>((_, reject) => {
			timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
		});
		try {
			return await Promise.race([promise, timeout]);
		} finally {
			if (timer) clearTimeout(timer);
		}
	};
	const attachChannel = async (path: string) => {
		debug(`opening readable: ${path}`);
		const stream = await withTimeout(wx.openReadable(path), 7000, `openReadable(${path})`);
		debug(`openReadable ok: ${path}`);
		debug(`opening writable: ${path}`);
		const writable = await withTimeout(wx.openWritable(path), 7000, `openWritable(${path})`);
		debug(`openWritable ok: ${path}`);
		const channelWriter = writable.getWriter();
		return { stream, writer: channelWriter, path };
	};
	const waitFirstChunk = async (
		reader: ReadableStreamDefaultReader<Uint8Array>,
		path: string,
		ms: number,
	): Promise<Uint8Array | null> => {
		const timeoutToken = Symbol("first-chunk-timeout");
		const firstRead = reader.read();
		const raced = await Promise.race([
			firstRead,
			new Promise<typeof timeoutToken>((resolve) => {
				setTimeout(() => resolve(timeoutToken), ms);
			}),
		]);
		if (raced === timeoutToken) {
			debug(`no terminal output within ${ms}ms after attach on ${path}`);
			// Important: do not await cancel here. In some Wanix stream states,
			// cancel() can hang and block channel failover execution.
			reader.cancel("first-chunk-timeout").catch(() => {
				// Best effort cleanup.
			});
			return null;
		}
		if ((raced as ReadableStreamReadResult<Uint8Array>).done) {
			debug(`readable stream closed before first chunk on ${path}`);
			return null;
		}
		const bytes = (raced as ReadableStreamReadResult<Uint8Array>).value;
		debug(`first output chunk received (${bytes?.byteLength ?? 0} bytes) on ${path}`);
		return bytes;
	};
	let writer: WritableStreamDefaultWriter<Uint8Array> | undefined;
	let writeQueue = Promise.resolve();
	const pty = {
		onDidWrite: writeEmitter.event,
		open: () => {
			(async () => {
				try {
					debug(`open() start; forceConsole=${String(forceConsoleChannel())}`);
					debug(`build marker: ${PTY_DEBUG_VERSION}`);
					await emitBridgeSelfTest();
					let dataPath = await resolveTerminalDataPathWithRetry(wx, writeEmitter);
					console.log("terminal channel", dataPath);
					writeEmitter.fire(`\r\n[apptron] terminal channel: ${dataPath}\r\n`);
					debug(`path resolved: ${dataPath}`);
					try {
						const st = await withTimeout(wx.stat(dataPath), 2500, `stat(${dataPath})`);
						debug(`stat(${dataPath}) ok type=${String(st?.type ?? "?")}`);
					} catch (e) {
						debug(`stat(${dataPath}) failed: ${String(e)}`);
					}
					let attached = await attachChannel(dataPath);
					writer = attached.writer;
					// Kick the prompt: BusyBox hush defers first prompt until it
					// sees input on stdin. Required for both #console/data and
					// web/dom/<id>/data channels.
					try {
						debug("writing initial newline kick");
						await withTimeout(writer.write(enc.encode("\r\n")), 3000, "initial newline write");
						debug("initial newline kick written");
					} catch {
						// Non-fatal — user keystrokes will still render the prompt.
						debug("initial newline kick failed (non-fatal)");
					}
					let reader = attached.stream.getReader();
					let firstChunk = await waitFirstChunk(reader, dataPath, 5000);
						const probeShellReadiness = async (
							reader: ReadableStreamDefaultReader<Uint8Array>,
							channelWriter: WritableStreamDefaultWriter<Uint8Array>,
							path: string,
							ms: number,
						): Promise<boolean> => {
							const token = `__APPTRON_PTY_READY_${Date.now()}__`;
							try {
								debug(`writing readiness probe on ${path}`);
								await withTimeout(channelWriter.write(enc.encode(`echo ${token}\r\n`)), 3000, `probe write ${path}`);
							} catch (e) {
								debug(`readiness probe write failed on ${path}: ${String(e)}`);
								return false;
							}

							const timeoutToken = Symbol("probe-timeout");
							let seen = "";
							while (true) {
								const raced = await Promise.race([
									reader.read(),
									new Promise<typeof timeoutToken>((resolve) => {
										setTimeout(() => resolve(timeoutToken), ms);
									}),
								]);
								if (raced === timeoutToken) {
									debug(`readiness probe timed out after ${ms}ms on ${path}`);
									return false;
								}
								const { done, value } = raced as ReadableStreamReadResult<Uint8Array>;
								if (done) {
									debug(`readiness probe stream closed on ${path}`);
									return false;
								}
								const text = dec.decode(value);
								seen += text;
								writeEmitter.fire(text);
								if (seen.includes(token)) {
									debug(`readiness probe succeeded on ${path}`);
									return true;
								}
							}
						};
						let channelReady = false;
					if (!firstChunk && dataPath !== "#console/data") {
						debug("first-chunk timeout on primary channel; entering fallback branch");
						debug(`fallback probe: switching channel to #console/data`);
						try {
							await writer.close();
						} catch {
							// Ignore close errors during failover.
						}
						attached = await attachChannel("#console/data");
						writer = attached.writer;
						dataPath = "#console/data";
						debug(`fallback channel attached: ${dataPath}`);
						writeEmitter.fire(`\r\n[apptron] fallback terminal channel: ${dataPath}\r\n`);
						try {
							debug("writing fallback newline kick");
							await withTimeout(writer.write(enc.encode("\r\n")), 3000, "fallback newline write");
							debug("fallback newline kick written");
						} catch {
							debug("fallback newline kick failed (non-fatal)");
						}
						reader = attached.stream.getReader();
						firstChunk = await waitFirstChunk(reader, dataPath, 5000);
					}
					if (firstChunk) {
						writeEmitter.fire(dec.decode(firstChunk));
						debug(`post-first-chunk: entering readiness probe on ${dataPath}`);
							channelReady = await probeShellReadiness(reader, writer, dataPath, 3000);
					}
					try {
						while (true) {
							const { done, value } = await reader.read();
							if (done) {
								debug("readable stream closed (done=true)");
								break;
							}
							writeEmitter.fire(dec.decode(value));
						}
					} finally {
						reader.releaseLock();
					}
				} catch (error) {
					debug(`open/read pipeline failed: ${String(error)}`);
					console.error("terminal bridge open/read failed", error);
				}
			})();
		},
		close: () => {
			if (writer) {
				void writer.close().catch((error: unknown) => {
					console.error("terminal writer close failed", error);
				});
				writer = undefined;
			}
		},
		handleInput: (data: string) => {
			if (!writer) {
				return;
			}
			const payload = enc.encode(data);
			writeQueue = writeQueue.then(async () => {
				await writer!.write(payload);
			}).catch((error: unknown) => {
				console.error("terminal write failed", error);
			});
		}
	};
	return vscode.window.createTerminal({ name: `Wanix Shell`, pty });
}


// @ts-ignore
// polyfill for ReadableStream.prototype[Symbol.asyncIterator] on safari
if (!ReadableStream.prototype[Symbol.asyncIterator]) {
	// @ts-ignore
    ReadableStream.prototype[Symbol.asyncIterator] = async function* () {
        const reader = this.getReader();
        try {
            while (true) {
                const { done, value } = await reader.read();
                if (done) return;
                yield value;
            }
        } finally {
            reader.releaseLock();
        }
    };
}

class WebViewPanelSerializer implements vscode.WebviewPanelSerializer {
	async deserializeWebviewPanel(webviewPanel: vscode.WebviewPanel, state: any) {
	  // Restore the content of our webview.
	  webviewPanel.webview.html = monitorHtml;
	  webviewPanel.webview.postMessage({ origin: location.origin, vm: state.vm });
	}
  }