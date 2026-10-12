/**
 * Theme: writes dynamic CSS properties to :root.
 */


/** Apply reduce-motion preference as a data attribute the CSS can key off. */
export function applyReduceMotion(reduce: boolean): void {
  document.documentElement.dataset.motion = reduce ? 'reduce' : 'full'
}
import {contrastInk, validHex} from '../../../../src/palette'
import '../styles/library-theme.css'

export function applyBackgroundColor(value?: string): void {
  const color=validHex(value)?value:'#000000'
  const root=document.documentElement
  const ink=contrastInk(color)
  const rgb=[1,3,5].map(index=>parseInt(color.slice(index,index+2),16))
  const inkRgb=ink==='#ffffff'?[255,255,255]:[23,23,23]
  const mix=(amount:number)=>`rgb(${rgb.map((channel,index)=>Math.round(channel*(1-amount)+inkRgb[index]*amount)).join(', ')})`
  const values:Record<string,string>={
    '--bg-0':color,'--bg-1':color,'--bg-2':mix(.08),'--bg-3':mix(.14),
    '--bg-card':mix(.08),'--bg-card-hover':mix(.14),'--bg-chip':mix(.08),'--bg-glass':mix(.06),
    '--text-primary':ink,'--text-secondary':`rgba(${inkRgb.join(',')},.78)`,
    '--text-tertiary':`rgba(${inkRgb.join(',')},.62)`,
    '--edge-light':`rgba(${inkRgb.join(',')},.08)`,'--edge-strong':`rgba(${inkRgb.join(',')},.18)`,
    '--divider':`rgba(${inkRgb.join(',')},.12)`,
  }
  if(color.toLowerCase()==='#000000') {
    delete root.dataset.libraryColor
    Object.keys(values).forEach(key=>root.style.removeProperty(key))
  } else {
    root.dataset.libraryColor=color
    Object.entries(values).forEach(([key,value])=>root.style.setProperty(key,value))
  }
}
