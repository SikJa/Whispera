import {paint,type Mark} from './screen-annotations';

// Only dense drawings benefit from an extra surface. Keep at most one 32 MiB
// RGBA layer per editor; larger captures retain the original vector path.
export class InkLayer {
  private canvas: HTMLCanvasElement | undefined;
  private marks: Mark[] | undefined;
  private key='';
  private excluded: string | undefined;
  private background: CanvasImageSource | null=null;

  clear() {
    if(this.canvas){this.canvas.width=0;this.canvas.height=0;}
    this.canvas=undefined;this.marks=undefined;this.background=null;
  }

  render(marks:Mark[],excluded:string|undefined,width:number,height:number,scale:number,
    key:string,background:CanvasImageSource|null,paintBackground:(ctx:CanvasRenderingContext2D)=>void) {
    const w=Math.round(width*scale),h=Math.round(height*scale);
    let points=0;
    for(const mark of marks){points+=mark.points.length;if(points>=512)break;}
    if(points<512 || w*h>8_388_608 || w<=0 || h<=0){this.clear();return;}
    if(this.canvas && this.marks===marks && this.key===key && this.excluded===excluded && this.background===background)return this.canvas;
    const canvas=this.canvas??document.createElement('canvas');
    if(canvas.width!==w)canvas.width=w;
    if(canvas.height!==h)canvas.height=h;
    const ctx=canvas.getContext('2d');
    if(!ctx){this.clear();return;}
    ctx.setTransform(scale,0,0,scale,0,0);ctx.clearRect(0,0,width,height);
    paintBackground(ctx);
    for(const mark of marks)if(mark.id!==excluded)paint(ctx,mark);
    this.canvas=canvas;this.marks=marks;this.key=key;this.excluded=excluded;this.background=background;
    return canvas;
  }
}
