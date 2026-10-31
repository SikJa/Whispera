import assert from 'node:assert/strict';
import {mkdir, mkdtemp, readFile, realpath, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
import {installNativeMock} from './tests/native-mock.mjs';

// Every native effect stays inside this fixture, including permission requests.
function installPlatformFixture(platform) {
  Object.defineProperty(navigator, 'platform', {get: () => platform});
  const mac = platform === 'MacIntel';
  window.voiceShortcut = mac ? 'Super+Shift+KeyD' : 'Control+Shift+KeyD';
  window.mockPermissions = {microphone: 'required', screen: 'granted', accessibility: 'denied'};
  window.assetCalls = [];
  window.mediaPlayCalls = 0;
  HTMLMediaElement.prototype.play = function () { window.mediaPlayCalls++; return Promise.resolve(); };
  window.__TAURI_INTERNALS__.convertFileSrc = (path, protocol) => {
    window.assetCalls.push({path, protocol});
    return 'data:video/mp4;base64,';
  };
  const invoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (command, args = {}, options) => {
    const intercepted = ['platform_permissions', 'platform_request_permission', 'startup_enabled', 'set_startup', 'windows_capture_shortcuts', 'video_trim_context', 'video_trim_save', 'read_history'];
    if (intercepted.includes(command)) window.calls.push({command, args});
    if (command === 'platform_permissions') return {...window.mockPermissions};
    if (command === 'platform_request_permission') {
      window.mockPermissions[args.kind] = 'granted';
      return {...window.mockPermissions};
    }
    if (command === 'startup_enabled') return false;
    if (command === 'set_startup') return args.enabled;
    if (command === 'windows_capture_shortcuts') {
      if (mac) throw Error('Windows integration must not be queried on macOS');
      return false;
    }
    if (command === 'video_trim_context') return {path: mac ? '/tmp/fixture/clip.mp4' : 'C:\\fixture\\clip.mp4', name: 'clip.mp4', duration: 60, hasAudio: false};
    if (command === 'video_trim_save') return {path: 'fixture-trim.mp4', transcript: false, warning: ''};
    if (command === 'read_history') return [];
    const result = await invoke(command, args, options);
    if (command === 'snapshot') return {...result, keyConfigured: true, settings: {
      model: 'whisper-large-v3-turbo', pattern: 'wave', placement: 'right', autoCopy: true,
      soundTheme: 'cristal', sounds: false, recorderScale: .85, trimSilence: true,
      watchdog: false, incrementalTranscription: true, ...result.settings,
    }};
    return result;
  };
}

const root = fileURLToPath(new URL('.', import.meta.url));
const cacheDir = await mkdtemp(join(tmpdir(), 'whispera-macos-ui-'));
const screenshots = join(root, 'test-results', 'macos-ui');
let server, browser;
const errors = [];
try {
  let base = process.env.WHISPERA_TEST_URL;
  if (!base) {
    const dependencies = await realpath(new URL('./node_modules', import.meta.url));
    server = await createServer({root, cacheDir, logLevel: 'silent', server: {host: '127.0.0.1', port: 0, strictPort: false, open: false, hmr: false, fs: {allow: [root, dependencies]}}});
    await server.listen();
    base = `http://127.0.0.1:${server.httpServer.address().port}`;
  }
  const origin = new URL(base).origin;
  assert.ok(['localhost', '127.0.0.1', '[::1]'].includes(new URL(base).hostname), 'Fixtures only run on a local server');
  browser = await launchSilentBrowser();
  await mkdir(screenshots, {recursive: true});
  async function pageFor(platform, options = {}) {
    const page = await browser.newPage({viewport: {width: 900, height: 720}, locale: 'es-AR'});
    page.setDefaultTimeout(15000);
    page.on('pageerror', error => errors.push(`${platform}: ${error.message}`));
    await page.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
    await page.addInitScript({content: `(${installNativeMock.toString()})(${JSON.stringify(options)}); (${installPlatformFixture.toString()})(${JSON.stringify(platform)});`});
    return page;
  }
  const calls = (page, command) => page.evaluate(command => window.calls.filter(call => call.command === command), command);
  async function noEffects(page) {
    assert.equal(await page.evaluate(() => window.mediaPlayCalls), 0, 'No media playback was requested');
    assert.deepEqual(await page.evaluate(() => window.calls.filter(call => ['restart_app', 'updater_install', 'recording_action', 'preview_sound', 'play_sound'].includes(call.command))), []);
    if (await page.evaluate(() => navigator.platform === 'MacIntel')) {
      assert.deepEqual(await page.evaluate(() => window.calls.filter(call => call.command.includes('windows_capture_shortcuts'))), [], 'No Windows capture hooks on macOS');
    }
  }
  async function fits(page, selector) {
    assert.equal(await page.evaluate(async () => {
      await document.fonts.load('15px "Instrument Sans Variable"');
      return document.fonts.check('15px "Instrument Sans Variable"');
    }), true, 'Layout checks use the actual application font');
    assert.equal(await page.locator(selector).evaluate(element => element.scrollWidth <= element.clientWidth + 1), true, `${selector} must not overflow horizontally`);
  }
  async function permissionScreenshots(page, surface, lang = 'es') {
    const permissions = page.getByRole('region', {name: lang === 'es' ? 'Permisos de macOS' : 'macOS permissions'});
    await permissions.getByRole('status').waitFor();
    for (const width of [620, 900]) {
      await page.setViewportSize({width, height: 720});
      await fits(page, surface === 'setup' ? '.setup-shell' : '.desktop-main');
      await fits(page, `section[aria-label="${lang === 'es' ? 'Permisos de macOS' : 'macOS permissions'}"]`);
      assert.deepEqual(await permissions.locator('.form-row').evaluateAll(rows => rows.flatMap((row, index) => {
        const bounds = row.getBoundingClientRect();
        const children = [...row.children].map(child => child.getBoundingClientRect());
        const failures = [];
        for (const [i, rect] of children.entries()) {
          if (rect.left < bounds.left - 1 || rect.right > bounds.right + 1) failures.push(`row ${index}, child ${i}: outside row`);
          for (const next of children.slice(i + 1)) {
            if (Math.min(rect.right, next.right) - Math.max(rect.left, next.left) > 1 &&
                Math.min(rect.bottom, next.bottom) - Math.max(rect.top, next.top) > 1) failures.push(`row ${index}: overlapping controls`);
          }
        }
        return failures;
      })), [], `${surface} permissions fit without overlapping at ${width}px`);
      await permissions.evaluate(element => element.scrollIntoView({block: 'center'}));
      await page.screenshot({path: join(screenshots, `${surface}-permissions-${lang}-${width}.png`), fullPage: true, animations: 'disabled'});
    }
  }

  for (const platform of ['MacIntel', 'Win32']) {
    const mac = platform === 'MacIntel', key = mac ? 'Cmd' : 'Ctrl';
    const page = await pageFor(platform, {setupComplete: false});
    await page.goto(base);
    await page.getByRole('heading', {name: 'Tu voz, tu configuraci\u00f3n'}).waitFor();
    assert.match(await page.locator('.setup-step').innerText(), mac ? /Keychain de macOS/ : /credenciales de Windows/);
    if (mac) assert.doesNotMatch(await page.locator('.setup-shell').innerText(), /Windows/);
    await page.getByLabel('Language / Idioma').selectOption('en');
    assert.match(await page.locator('.setup-step').innerText(), mac ? /macOS Keychain/ : /Windows Credential Manager/);
    await page.getByRole('button', {name: 'Continue', exact: true}).click();
    await page.getByText(mac ? 'Key saved in macOS Keychain' : 'Key saved in Windows', {exact: true}).waitFor();
    await page.getByLabel('Language / Idioma').selectOption('es');
    await page.getByText(mac ? 'Clave guardada en Keychain de macOS' : 'Clave guardada en Windows', {exact: true}).waitFor();
    await page.getByRole('button', {name: 'Continuar', exact: true}).click();
    await page.getByRole('heading', {name: 'Prepar\u00e1 tu primer dictado'}).waitFor();
    assert.match(await page.locator('#setup-hotkey').inputValue(), new RegExp(`${key} \\+ Shift \\+ D`));
    assert.match(await page.locator('.setup-step').innerText(), new RegExp(`Micr.fono predeterminado de ${mac ? 'macOS' : 'Windows'}:`));
    await page.getByLabel('Language / Idioma').selectOption('en');
    if (mac) {
      const permissions = page.getByRole('region', {name: 'macOS permissions'});
      await permissions.locator('label').filter({hasText: 'Microphone'}).waitFor();
      await permissions.locator('label').filter({hasText: 'Screen recording'}).waitFor();
      await permissions.locator('label').filter({hasText: 'Accessibility'}).waitFor();
      await permissionScreenshots(page, 'setup', 'en');
      await page.getByLabel('Language / Idioma').selectOption('es');
      await permissionScreenshots(page, 'setup');
      await page.getByLabel('Language / Idioma').selectOption('en');
      await permissions.locator('.form-row').filter({hasText: 'Microphone'}).getByRole('button', {name: 'Allow', exact: true}).click();
      await permissions.locator('.form-row').filter({hasText: 'Accessibility'}).getByRole('button', {name: 'Allow', exact: true}).click();
      assert.deepEqual((await calls(page, 'platform_request_permission')).map(call => call.args), [{kind: 'microphone', settings: false}, {kind: 'accessibility', settings: true}]);
      await permissions.getByRole('button', {name: 'Check permissions', exact: true}).click();
      await page.getByLabel('Language / Idioma').selectOption('es');
      await page.getByRole('region', {name: 'Permisos de macOS'}).locator('label').filter({hasText: 'Accesibilidad'}).waitFor();
      assert.doesNotMatch(await page.locator('.setup-step').innerText(), /Windows/);
    } else {
      assert.equal(await page.getByRole('region', {name: /macOS/}).count(), 0);
      assert.deepEqual(await calls(page, 'platform_permissions'), []);
    }
    for (const width of [620, 900]) {
      await page.setViewportSize({width, height: 720});
      await fits(page, '.setup-shell');
    }
    await page.getByLabel('Language / Idioma').selectOption('es');
    await page.getByRole('button', {name: 'Continuar', exact: true}).click();
    await page.getByRole('heading', {name: 'Listo para dictar', exact: true}).waitFor();
    assert.deepEqual((await calls(page, 'set_startup')).map(call => call.args), [{enabled: false}]);
    for (const lang of ['es', 'en']) {
      await page.getByLabel('Language / Idioma').selectOption(lang);
      const finalLabel = lang === 'es' ? mac ? 'Ir a la barra de men\u00fas' : 'Ir a la bandeja' : mac ? 'Continue in menu bar' : 'Continue in tray';
      const finish = page.getByRole('button', {name: finalLabel, exact: true});
      await finish.waitFor();
      if (mac) assert.doesNotMatch(await page.locator('.setup-shell').innerText(), /Windows|bandeja|tray/);
      for (const width of [620, 900]) {
        await page.setViewportSize({width, height: 720});
        await finish.scrollIntoViewIfNeeded();
        await fits(page, '.setup-shell');
        assert.equal(await finish.evaluate(button => {
          const rect = button.getBoundingClientRect();
          return rect.top >= 0 && rect.bottom <= innerHeight && rect.left >= 0 && rect.right <= innerWidth;
        }), true, 'The final setup action remains reachable without completing setup');
      }
    }
    await noEffects(page);
    await page.close();

    const settings = await pageFor(platform);
    await settings.goto(base);
    await settings.getByText(`Almacenada en ${mac ? 'Keychain de macOS' : 'Windows'}`, {exact: true}).waitFor();
    await settings.locator('#key').fill('gsk_fixture_only');
    await settings.locator('#key').blur();
    await settings.getByText(`Clave guardada autom\u00e1ticamente en ${mac ? 'Keychain de macOS' : 'Windows'}`, {exact: true}).waitFor();
    assert.equal((await calls(settings, 'save_api_key')).length, 1);
    await settings.getByRole('button', {name: 'Aplicaci\u00f3n', exact: true}).click();
    if (mac) await permissionScreenshots(settings, 'settings');
    else {
      assert.equal(await settings.getByRole('region', {name: /macOS/}).count(), 0);
      assert.deepEqual(await calls(settings, 'platform_permissions'), []);
    }
    await settings.getByRole('button', {name: 'Apariencia', exact: true}).click();
    await settings.getByRole('heading', {name: mac ? 'Icono de barra de men\u00fas \u00b7 Cristal' : 'Icono de bandeja \u00b7 Cristal', exact: true}).waitFor();
    await settings.evaluate(() => {window.videoStatus.path = 'fixture.mp4'; window.videoStatus.copied = true;});
    await settings.getByRole('button', {name: 'Capturas y video', exact: true}).click();
    await settings.getByText(`Micr\u00f3fono de ${mac ? 'macOS' : 'Windows'}`, {exact: true}).waitFor();
    await settings.getByText(`Video copiado. Pegalo con ${key}+V.`, {exact: true}).waitFor();
    assert.match(await settings.locator('.capture-shortcuts').innerText(), new RegExp(`${key}\\+Z.*${key}\\+Shift\\+Z`));
    assert.equal(await settings.locator('#windows-capture-shortcuts').count(), mac ? 0 : 1);
    assert.equal((await calls(settings, 'windows_capture_shortcuts')).length, mac ? 0 : 1);
    if (mac) assert.doesNotMatch(await settings.locator('.screen-recorder-panel').innerText(), /Windows|Impr Pant|Ctrl\+/);
    for (const width of [620, 900]) {
      await settings.setViewportSize({width, height: 720});
      await fits(settings, '.desktop-main');
    }
    await settings.getByRole('button', {name: 'Historial', exact: true}).click();
    await settings.getByRole('button', {name: 'Capturas y videos', exact: true}).click();
    await settings.getByRole('button', {name: 'Copiar', exact: true}).click();
    await settings.getByText(`Copiado. Pegalo con ${key}+V.`, {exact: true}).waitFor();
    await noEffects(settings);
    await settings.close();

    for (const modifier of mac ? ['Meta', 'Control'] : ['Control']) {
      const selection = await pageFor(platform, {kind: 'image'});
      await selection.setViewportSize({width: 640, height: 480});
      await selection.goto(`${base}/overlay.html?view=screen-select`);
      await selection.getByText(`${key}+A: pantalla completa`, {exact: true}).waitFor();
      await selection.mouse.move(60, 100);
      await selection.mouse.down();
      await selection.mouse.move(220, 210);
      await selection.locator('.screen-selection-rect').waitFor();
      await selection.keyboard.press(`${modifier}+z`);
      await selection.locator('.screen-selection-rect').waitFor({state: 'detached'});
      await selection.mouse.up();
      assert.equal((await calls(selection, 'screen_start')).length, 0, 'Undo cancels the selection before capture');
      await selection.keyboard.press(`${modifier}+a`);
      await selection.waitForFunction(() => window.calls.some(call => call.command === 'screen_start'));
      assert.deepEqual((await calls(selection, 'screen_start')).map(call => call.args.rect), [{x: 0, y: 0, width: 640, height: 480}]);
      await noEffects(selection);
      await selection.close();
    }

    const ink = await pageFor(platform, {kind: 'image'});
    await ink.setViewportSize({width: 640, height: 480});
    await ink.goto(`${base}/overlay.html?view=screen-ink`);
    await ink.waitForFunction(() => window.calls.some(call => call.command === 'screen_editor_ready') && !window.feedback.busy);
    await ink.keyboard.press('a');
    await ink.mouse.move(50, 50);
    await ink.mouse.down();
    await ink.mouse.move(200, 120);
    await ink.mouse.up();
    await ink.waitForFunction(() => window.feedback.count === 1);
    if (!mac) {
      await ink.keyboard.press('Meta+z');
      await ink.keyboard.press('Meta+c');
      await ink.keyboard.press('Meta+p');
      await ink.evaluate(() => new Promise(requestAnimationFrame));
      assert.equal(await ink.evaluate(() => window.feedback.count), 1, 'Windows key combinations never undo annotations');
      assert.equal(await ink.evaluate(() => window.feedback.tool), 'arrow', 'Windows key combinations never select tools');
      assert.deepEqual(await calls(ink, 'screen_image_export'), [], 'Windows key combinations never export images');
    }
    for (const modifier of mac ? ['Meta', 'Control'] : ['Control']) {
      await ink.keyboard.press(`${modifier}+z`);
      await ink.waitForFunction(() => window.feedback.count === 0 && window.feedback.canRedo);
      await ink.keyboard.press(`${modifier}+Shift+z`);
      await ink.waitForFunction(() => window.feedback.count === 1);
      const before = (await calls(ink, 'screen_image_export')).length;
      await ink.keyboard.press(`${modifier}+c`);
      await ink.waitForFunction(count => window.exports.length === count && !window.feedback.busy, before + 1);
      await ink.keyboard.press(`${modifier}+s`);
      await ink.waitForFunction(count => window.exports.length === count && !window.feedback.busy, before + 2);
      assert.deepEqual((await calls(ink, 'screen_image_export')).slice(-2).map(call => call.args.action), ['copy', 'save']);
    }
    await noEffects(ink);
    await ink.goto(`${base}/overlay.html?view=screen-hud`);
    await ink.setViewportSize({width: 297, height: 56});
    await ink.getByRole('button', {name: 'Copiar', exact: true}).waitFor();
    assert.equal(await ink.getByRole('button', {name: 'Copiar', exact: true}).getAttribute('title'), `Copiar (${key}+C)`);
    assert.equal(await ink.getByLabel('Guardar imagen').getAttribute('title'), `Guardar PNG (${key}+S)`);
    await fits(ink, '.capture-image-actions');
    await ink.close();

    const trim = await pageFor(platform);
    await trim.setViewportSize({width: 520, height: 440});
    await trim.goto(`${base}/?view=video-trim`);
    await trim.getByText(`Tambi\u00e9n ${key}+C \u00b7 copia para pegar`, {exact: true}).waitFor();
    await fits(trim, '.trim-editor');
    await trim.getByRole('spinbutton', {name: 'Inicio del tramo'}).fill('12');
    await trim.getByRole('spinbutton', {name: 'Fin del tramo'}).fill('24');
    for (const modifier of mac ? ['Meta', 'Control'] : ['Control']) {
      const before = (await calls(trim, 'video_trim_save')).length;
      await trim.getByRole('spinbutton', {name: 'Fin del tramo'}).focus();
      await trim.keyboard.press(`${modifier}+c`);
      assert.equal((await calls(trim, 'video_trim_save')).length, before, 'Copy in a numeric input never exports');
      await trim.getByRole('slider', {name: 'Seleccionar final', exact: true}).focus();
      await trim.keyboard.press(`${modifier}+c`);
      await trim.waitForFunction(count => window.calls.filter(call => call.command === 'video_trim_save').length === count, before + 1);
      await trim.getByText(`Tramo guardado y copiado. Pegalo con ${key}+V.`, {exact: true}).waitFor();
      assert.deepEqual((await calls(trim, 'video_trim_save')).at(-1).args, {start: 12, end: 24});
    }
    const assets = await trim.evaluate(() => window.assetCalls);
    assert.ok(assets.length > 0);
    assert.ok(assets.every(asset => asset.protocol === 'asset' && asset.path === (mac ? '/tmp/fixture/clip.mp4' : 'C:\\fixture\\clip.mp4')), 'Media uses Tauri file conversion without Windows-only path rewriting');
    await noEffects(trim);
    await trim.close();
  }

  const config = JSON.parse(await readFile(new URL('./src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
  const directives = new Map(config.app.security.csp.split(';').map(directive => {
    const [name, ...values] = directive.trim().split(/\s+/);
    return [name, values];
  }));
  for (const directive of ['img-src', 'media-src']) assert.ok(directives.get(directive)?.includes('asset:'), `${directive} permits macOS asset URLs`);
  assert.deepEqual(errors, []);
  console.log('PASS: macOS/Windows setup ES+EN, Keychain/system labels, mocked permission requests, Windows-only integration gating, Cmd/Ctrl selection/undo/redo/copy, trim input guards, native-sized layout bounds and asset/CSP contracts. Headless, silent, mocked IPC only; no native macOS verification.');
  console.log(`Permission screenshots (620/900 px): ${screenshots}`);
} finally {
  await browser?.close();
  await server?.close();
  assert.equal(dirname(resolve(cacheDir)), resolve(tmpdir()), 'Only remove the fixture-owned temporary cache');
  await rm(cacheDir, {recursive: true, force: true});
}
