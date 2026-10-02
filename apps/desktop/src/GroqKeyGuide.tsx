import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import { native } from './client';
import './groq-guide.css';

export default function GroqKeyGuide({ lang = 'es', expanded = false }: { lang?: 'es' | 'en'; expanded?: boolean }) {
  const [error,setError]=useState('');
  const t = (es: string, en: string) => lang === 'es' ? es : en;
  const steps = [
    [t('Crear una cuenta', 'Create an account'), t('Ingresá a la consola de Groq y creá tu cuenta o iniciá sesión.', 'Open the Groq console and create an account or sign in.'), '01-login.png'],
    [t('Ir a API Keys', 'Open API Keys'), t('En el menú de la consola, entrá a API Keys.', 'In the console menu, open API Keys.'), '02-api-keys.png'],
    [t('Crear una nueva clave', 'Create a new key'), t('Hacé clic en Create API Key.', 'Click Create API Key.'), '03-create-key.png'],
    [t('Ponerle un nombre', 'Name your key'), t('Usá un nombre que reconozcas, por ejemplo Whispera.', 'Use a name you recognize, such as Whispera.'), '04-name-key.png'],
    [t('Copiar y guardar', 'Copy and save'), t('Copiá la clave, pegala en el campo de esta pantalla y guardala. Empieza con gsk_ y Groq la muestra una sola vez.', 'Copy the key, paste it into the field on this page and save it. It starts with gsk_ and Groq only shows it once.'), '05-copy-key.png'],
  ];
  return <details className="groq-guide" open={expanded || undefined}>
    <summary>{t('¿Cómo obtener tu clave de Groq?', 'How do I get a Groq key?')}</summary>
    <div className="groq-guide-content">
      <a className="groq-console-link" href="https://console.groq.com/keys" target="_blank" rel="noreferrer" onClick={e => {
        if (native) { e.preventDefault(); void invoke('open_groq_console').catch(e=>setError(String(e))); }
      }}>{t('Abrir la consola de Groq ↗', 'Open the Groq console ↗')}</a>
      {error&&<p role="alert">{error}</p>}
      {steps.map(([title, description, file], index) => <div className="guide-step" key={file}>
        <strong>{index + 1}. {title}</strong><p>{description}</p>
        <img src={`/groq-setup/${file}`} alt={title} loading="lazy" decoding="async" />
      </div>)}
    </div>
  </details>;
}
