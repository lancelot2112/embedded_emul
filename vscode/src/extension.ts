import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const serverOptions = buildServerOptions(context);
  const traceOutputChannel = vscode.window.createOutputChannel(
    "Nanemu ISA Trace",
  );
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "nanemu-isa" },
      { scheme: "untitled", language: "nanemu-isa" },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher(
        "**/*.{isa,isaext,coredef,sysdef}",
      ),
    },
    outputChannelName: "Nanemu ISA Language Server",
    traceOutputChannel,
  };

  client = new LanguageClient(
    "nanemuIsaLanguageServer",
    "Nanemu ISA Language Server",
    serverOptions,
    clientOptions,
  );
  client.registerProposedFeatures();
  client.start();
  context.subscriptions.push(client);
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

function buildServerOptions(
  context: vscode.ExtensionContext,
): ServerOptions {
  const config = vscode.workspace.getConfiguration("nanemu.isaLanguageServer");
  const customCommand = config.get<string>("serverCommand")?.trim();
  const customArgs = config.get<string[]>("serverArgs") ?? [];
  const cwd = getWorkspaceDirectory(context);
  const bundledServer = context.asAbsolutePath(
    path.join("server", serverBinaryName()),
  );

  if (customCommand) {
    vscode.window.showInformationMessage(
      `Starting Nanemu ISA language server: ${customCommand} ${customArgs.join(" ")}`,
    );
    return {
      command: customCommand,
      args: customArgs,
      options: { cwd },
    };
  }

  if (fs.existsSync(bundledServer)) {
    vscode.window.showInformationMessage(
      `Starting Nanemu ISA language server from bundled binary: ${bundledServer}`,
    );
    return {
      command: bundledServer,
      args: [],
    };
  }

  const defaultArgs = [
    "run",
    "--features",
    "language-server",
    "--bin",
    "isa_language_server",
  ];

  vscode.window.showInformationMessage(
    "Starting Nanemu ISA language server via 'cargo run --features language-server --bin isa_language_server'",
  );

  return {
    command: "cargo",
    args: defaultArgs,
    options: { cwd },
  };
}

function getWorkspaceDirectory(
  context: vscode.ExtensionContext,
): string | undefined {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (folder) {
    return folder.uri.fsPath;
  }
  return path.dirname(context.extensionPath);
}

function serverBinaryName(): string {
  return process.platform === "win32"
    ? "isa_language_server.exe"
    : "isa_language_server";
}
