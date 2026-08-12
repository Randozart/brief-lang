const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
const fs = require('fs');
const path = require('path');
const os = require('os');

let client;

function findBriev(context) {
    // 1. Try bundled binary (most reliable for Flatpak/Snap)
    const bundledPath = path.join(context.extensionPath, 'client', 'bin', 'briev');
    if (fs.existsSync(bundledPath)) {
        return bundledPath;
    }

    // 2. Try common locations
    const home = os.homedir();
    const commonPaths = [
        path.join(home, '.local/bin/briev'),
        path.join(home, 'bin/briev'),
        '/usr/local/bin/briev',
        '/usr/bin/briev'
    ];

    for (const p of commonPaths) {
        if (fs.existsSync(p)) {
            return p;
        }
    }

    // 3. Try PATH as a last resort
    return 'briev';
}

function activate(context) {
    const logPath = path.join(os.tmpdir(), 'briev-extension.log');
    fs.appendFileSync(logPath, 'Briev extension activate called\n');

    const brievPath = findBriev(context);
    fs.appendFileSync(logPath, `Using Briev binary at: ${brievPath}\n`);

    // The server is implemented in the briev binary
    const serverOptions = {
        run: { command: brievPath, args: ['lsp'], transport: TransportKind.stdio },
        debug: { command: brievPath, args: ['lsp'], transport: TransportKind.stdio }
    };

    // Options to control the language client
    const clientOptions = {
        // Register the server for Briev, DBriev, and Strict Briev files
        documentSelector: [
            { scheme: 'file', language: 'briev' },
            { scheme: 'file', language: 'rbv' },
            { scheme: 'file', language: 'ebv' },
            { scheme: 'file', language: 'dbriev' },
            { scheme: 'file', language: 'sbriev' },
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
        'brievLanguageServer',
        'Briev Language Server',
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
