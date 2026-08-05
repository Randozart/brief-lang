const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
const fs = require('fs');
const path = require('path');
const os = require('os');

let client;

function findBriv(context) {
    // 1. Try bundled binary (most reliable for Flatpak/Snap)
    const bundledPath = path.join(context.extensionPath, 'client', 'bin', 'briv');
    if (fs.existsSync(bundledPath)) {
        return bundledPath;
    }

    // 2. Try common locations
    const home = os.homedir();
    const commonPaths = [
        path.join(home, '.local/bin/briv'),
        path.join(home, 'bin/briv'),
        '/usr/local/bin/briv',
        '/usr/bin/briv'
    ];

    for (const p of commonPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    // 3. Try PATH as a last resort
    return 'briv';
}

function activate(context) {
    const logPath = path.join(os.tmpdir(), 'briv-extension.log');
    fs.appendFileSync(logPath, 'Briv extension activate called\n');

    const brivPath = findBriv(context);
    fs.appendFileSync(logPath, `Using Briv binary at: ${brivPath}\n`);

    // The server is implemented in the briv binary
    const serverOptions = {
        run: { command: brivPath, args: ['lsp'], transport: TransportKind.stdio },
        debug: { command: brivPath, args: ['lsp'], transport: TransportKind.stdio }
    };

    // Options to control the language client
    const clientOptions = {
        // Register the server for Briv, DBriv, and Strict Briv files
        documentSelector: [
            { scheme: 'file', language: 'briv' },
            { scheme: 'file', language: 'rbv' },
            { scheme: 'file', language: 'ebv' },
            { scheme: 'file', language: 'dbriv' },
            { scheme: 'file', language: 'sbriv' },
            { scheme: 'file', language: 'srbv' },
            { scheme: 'file', language: 'sebv' }
        ],
        synchronize: {
            // Notify the server about file changes
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{bv,rbv,ebv,dbv,dbvl,sbv,srbv,sebv}')
        }
    };

    // Create the language client and start the client.
    client = new LanguageClient(
        'brivLanguageServer',
        'Briv Language Server',
        serverOptions,
        clientOptions
    );

    // Start the client. This will also launch the server
    client.start();
}

function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

module.exports = {
    activate,
    deactivate
};
