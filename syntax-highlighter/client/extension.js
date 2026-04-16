const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function activate(context) {
    console.log('Brief extension activated');

    // The server is implemented in the brief binary
    const serverOptions = {
        run: { command: 'brief', args: ['lsp'], transport: TransportKind.stdio },
        debug: { command: 'brief', args: ['lsp'], transport: TransportKind.stdio }
    };

    // Options to control the language client
    const clientOptions = {
        // Register the server for Brief files
        documentSelector: [
            { scheme: 'file', language: 'brief' },
            { scheme: 'file', language: 'rbv' },
            { scheme: 'file', language: 'ebv' }
        ],
        synchronize: {
            // Notify the server about file changes to '.bv', '.rbv' and '.ebv' files contained in the workspace
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.{bv,rbv,ebv}')
        }
    };

    // Create the language client and start the client.
    client = new LanguageClient(
        'briefLanguageServer',
        'Brief Language Server',
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
