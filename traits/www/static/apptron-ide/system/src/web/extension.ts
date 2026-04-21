
import * as vscode from 'vscode';
import { WanixBridge } from './bridge.js';
// @ts-ignore
import monitorHtml from "./monitor.html";

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
			// Non-fatal: direct web/dom path is still usable.
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
		return "#console/data";
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
	// Always prefer allocating a fresh xterm and reading from #console/data.
	// The iframe no longer pre-allocates one in ide_bridge mode, so there is
	// no active xterm DOM consumer competing for web/dom/<id>/data. Reading
	// from #console/data directly bypasses the alias routing entirely.
	const allocated = await tryAllocateXterm(wx);
	if (allocated) {
		return allocated;
	}
	// Fallback: if allocation fails but a prior id exists, use it.
	const existing = await tryFindExistingTerminalId(wx);
	if (existing) {
		return existing;
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
	let writer: WritableStreamDefaultWriter<Uint8Array> | undefined;
	let writeQueue = Promise.resolve();
	const pty = {
		onDidWrite: writeEmitter.event,
		open: () => {
			(async () => {
				try {
					const dataPath = await resolveTerminalDataPathWithRetry(wx, writeEmitter);
					console.log("terminal channel", dataPath);
					writeEmitter.fire(`\r\n[apptron] terminal channel: ${dataPath}\r\n`);
					const stream = await wx.openReadable(dataPath);
					const writable = await wx.openWritable(dataPath);
					writer = writable.getWriter();
					// Kick the prompt: BusyBox hush defers first prompt until it
					// sees input on stdin. Required for both #console/data and
					// web/dom/<id>/data channels.
					try {
						await writer.write(enc.encode("\n"));
					} catch {
						// Non-fatal — user keystrokes will still render the prompt.
					}
					for await (const chunk of stream) {
						writeEmitter.fire(dec.decode(chunk));
					}
				} catch (error) {
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