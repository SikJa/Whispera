import {native} from './client';
export default function InstalledVersion({version}:{version:string}) {
  return <section className="installed-version" id="installed-version" aria-label="Versión instalada">
    <div><h2>Whispera</h2><p>{native?'Versión instalada':'Vista previa en el navegador'}</p></div>
    <strong>{native?(version||'No disponible'):'Vista previa'}</strong>
  </section>;
}
