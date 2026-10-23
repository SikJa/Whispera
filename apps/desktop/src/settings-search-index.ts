// Local navigation metadata only: never index clipboard, history, credentials or account data.
export const sectionNames = {
  transcription:'Transcripción', reading:'Lectura', 'ai-usage':'Uso de IA', appearance:'Apariencia',
  hotkey:'Atajos', screen:'Capturas y video', sounds:'Sonidos', dictionary:'Diccionario personal',
  history:'Historial', library:'Portapapeles', application:'Aplicación', updates:'Actualizaciones', diagnostics:'Diagnóstico',
} as const;
export type SettingsRoute = keyof typeof sectionNames;
export type SettingResult = {route:SettingsRoute; title:string; target?:string; keywords:string; librarySettings?:boolean};
const item=(route:SettingsRoute,title:string,target:string,keywords='',librarySettings=false):SettingResult=>({route,title,target,keywords,librarySettings});
export const settingOptions:readonly SettingResult[] = [
  item('transcription','Modelo de transcripción','#model','groq whisper turbo'),
  item('transcription','Idioma del audio','#language','español ingles english detectar automatico dictado'),
  item('transcription','Clave API de Groq','#key','conexion contraseña credenciales token'),
  item('transcription','Copiar automáticamente','#copy','portapapeles al finalizar dictado'),
  item('transcription','Pegar en el destino original','#paste','automaticamente al finalizar dictado'),
  item('transcription','Transcribir mientras grabo','#incremental','anticipada incremental latencia'),
  item('transcription','Reducir silencios en el envío','#trim','recortar dictado'),
  item('transcription','Transcribir archivo','.page-actions','importar audio mp3 wav'),
  item('reading','Voz e idioma de lectura','[aria-label="Voz de lectura"]','voces español ingles english automatico detectar acento windows tts'),
  item('reading','Velocidad de lectura','[aria-label="Velocidad de lectura"]','rapidez lento rapido'),
  item('reading','Leer, pausar, continuar y detener','.reader-transport','reproducir texto hablar audio'),
  item('reading','Guardar lectura como MP3','.reader-transport','exportar descargar audio archivo'),
  item('ai-usage','Cuotas de OpenAI, Codex y Claude','.ai-providers','saldo restante uso limites suscripcion chatgpt anthropic ventanas'),
  item('ai-usage','Conectar Claude Code en Windows y WSL','.ai-connection','cli terminal cuotas suscripcion desconectar'),
  item('appearance','Tema de la aplicación','.app-theme-picker','oscuro colores ciruela grafito azul bosque pizarra arena'),
  item('appearance','Fondo personalizado','.app-background-picker','color fondo background'),
  item('appearance','Modelo de carpeta','.artwork-picker','animada metalica indicador'),
  item('appearance','Color de carpeta','.color-values','tinte indicador'),
  item('appearance','Movimiento de los papeles','#pattern','animacion ola escalera'),
  item('appearance','Tamaño de carpeta','#recorder-scale','escala indicador'),
  item('appearance','Posición de controles','#placement','arriba abajo izquierda derecha'),
  item('hotkey','Atajo de dictado','#shortcut-voice','shortcut teclado grabar voz'),
  item('hotkey','Atajo de video','#shortcut-video','shortcut teclado grabar pantalla'),
  item('hotkey','Atajo de captura de imagen','#shortcut-image','shortcut teclado screenshot'),
  item('hotkey','Atajo del portapapeles','#shortcut-library','shortcut teclado biblioteca'),
  item('hotkey','Atajo de lectura','#shortcut-reading','shortcut teclado leer texto seleccionado'),
  item('screen','Audio del video','[aria-label="Sonido del video"]','microfono sistema ambos sonido grabacion pantalla'),
  item('screen','Impr Pant para Whispera','#windows-capture-shortcuts','print screen captura windows'),
  item('screen','Copiar al soltar la selección','#image-auto-copy','captura imagen portapapeles editor'),
  item('screen','Color del recuadro','#screen-frame-color','marco captura video'),
  item('sounds','Sonidos de grabación','#sounds-on','activar desactivar silenciar dictado'),
  item('sounds','Sonido de inicio y fin','#sound-theme','tema grabacion dictado'),
  item('dictionary','Correcciones y palabras personales','.dictionary-add','reemplazar nombres sustituciones diccionario'),
  item('history','Historial de transcripciones, capturas y videos','.history-tabs','dictados anteriores buscar'),
  item('library','Color de fondo del portapapeles','.library-section fieldset','fondo personalizado biblioteca',true),
  item('library','Guardar lo copiado en otras aplicaciones','#library-captureGlobal','captura global privacidad historial clipboard',true),
  item('library','Pausar el historial','#library-incognito','incognito privacidad portapapeles',true),
  item('library','Capacidad del historial','#library-limit','limite elementos portapapeles',true),
  item('library','Eliminar automáticamente','#library-retention','borrar retencion dias horas portapapeles',true),
  item('library','Limpiar al reiniciar','#library-clearUnpinnedOnRestart','borrar historial fijados portapapeles',true),
  item('library','Transcribir el micrófono de los videos','#library-transcribeVideo','groq voz transcripcion audio',true),
  item('library','Incluir transcripción al copiar el video','#library-videoTranscriptAttachment','adjuntar texto txt',true),
  item('application','Iniciar Whispera al encender la computadora','#app-startup','inicio automatico windows arranque'),
  item('application','Versión instalada de Whispera','#installed-version','acerca de version build release actual'),
  item('updates','Actualizaciones manuales','.fork-update-info','actualizar instalar nueva version'),
  item('diagnostics','Recuperar interfaz sin respuesta','#watchdog','vigilancia reiniciar errores registros'),
];
const normalize=(value:string)=>value.normalize('NFD').replace(/[\u0300-\u036f]/g,'').toLocaleLowerCase('es').trim();
export function findSettings(query:string):SettingResult[] {
  const tokens=normalize(query).split(/\s+/).filter(Boolean);
  if(!tokens.length)return [];
  const sections:SettingResult[]=Object.entries(sectionNames).map(([route,title])=>({route:route as SettingsRoute,title,keywords:''}));
  return [...sections,...settingOptions].map((entry,index)=>{
    const title=normalize(entry.title),section=normalize(sectionNames[entry.route]);
    const text=`${title} ${section} ${normalize(entry.keywords)}`;
    return {entry,index,score:tokens.every(token=>text.includes(token))?tokens.reduce((sum,token)=>sum+(title.includes(token)?4:section.includes(token)?2:1),0):0};
  }).filter(result=>result.score>0).sort((a,b)=>b.score-a.score||a.index-b.index).map(result=>result.entry);
}
