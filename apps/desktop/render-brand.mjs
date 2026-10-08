import {readFileSync,mkdirSync} from 'node:fs';
import {launchSilentBrowser} from './tests/silent-browser.mjs';
const browser=await launchSilentBrowser();
try{
  mkdirSync('public/brand',{recursive:true});
  const page=await browser.newPage();
  await page.setContent(`<style>html,body{margin:0;padding:0;background:transparent}svg{display:block}</style>${readFileSync('public/brand/whispera-wave.svg','utf8')}`);
  for(const size of [16,24,32,48,64,128,256]){
    await page.locator('svg').evaluate((svg,size)=>{svg.setAttribute('width',String(size));svg.setAttribute('height',String(size));},size);
    await page.locator('svg').screenshot({path:`public/brand/${size}x${size}.png`,omitBackground:true});
  }
  console.log('Rendered wave logo at native icon sizes.');
}finally{await browser.close();}
