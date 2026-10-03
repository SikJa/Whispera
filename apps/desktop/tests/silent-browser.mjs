import { chromium } from 'playwright';

export async function launchSilentBrowser() {
  const browser = await chromium.launch({headless:true,args:['--mute-audio']});
  const newPage = browser.newPage.bind(browser);
  browser.newPage = async (...args) => {
    const page = await newPage(...args);
    await silencePage(page);
    return page;
  };
  return browser;
}

export async function silencePage(page) {
  await page.addInitScript(() => {
    // No media or Web Audio output may reach the stream's speakers during QA.
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function (...args) {
      this.volume = 0;
      this.muted = true;
      return play.apply(this, args);
    };
    if (window.AudioNode) {
      const connect = AudioNode.prototype.connect;
      AudioNode.prototype.connect = function (destination, ...args) {
        if (destination === this.context.destination) return destination;
        return connect.call(this, destination, ...args);
      };
    }
  });
}
