export const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
export const systemName = isMac ? 'macOS' : 'Windows';
export const commandKey = isMac ? 'Cmd' : 'Ctrl';
export const credentialStore = isMac ? 'Keychain de macOS' : 'Windows';
