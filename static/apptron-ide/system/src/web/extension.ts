
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

async function resolveTerminalDataPath(wx: any): Promise<string> {
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
		const frames = window.top?.document?.querySelectorAll("iframe") ?? [];
		for (const frame of Array.from(frames)) {
			const win = (frame as HTMLIFrameElement).contentWindow as any;
			const terminalId = String(win?.apptron?.terminalId || "").trim();
			if (/^[0-9]+$/.test(terminalId)) {
				return `web/dom/${terminalId}/data`;
			}
		}
	} catch {
		// same-origin/frame access can fail depending on host context
	}
	return "#console/data";
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
					const dataPath = await resolveTerminalDataPath(wx);
					console.log("terminal channel", dataPath);
					const stream = await wx.openReadable(dataPath);
					const writable = await wx.openWritable(dataPath);
					writer = writable.getWriter();
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