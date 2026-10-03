import {DatabaseSync} from 'node:sqlite';
import path from 'node:path';
const database=()=>new DatabaseSync(path.join(process.env.APPDATA,process.env.WHISPERA_TEST_IDENTIFIER??'app.whispera.desktop.preview','whispera.sqlite'));
export function snapshotCaptureHistory() {
  const db=database();try{return db.prepare('SELECT value FROM kv WHERE key=?').get('capture_history')?.value??null;}finally{db.close();}
}
export function restoreCaptureHistory(before,ownedFiles) {
  const db=database();
  try {
    db.exec('BEGIN IMMEDIATE');
    const original=JSON.parse(before??'[]'),current=JSON.parse(db.prepare('SELECT value FROM kv WHERE key=?').get('capture_history')?.value??'[]');
    const owned=new Set(ownedFiles.map(file=>file.toLowerCase()));
    const rows=new Map(original.map(row=>[row.path.toLowerCase(),row]));
    for(const row of current)if(!owned.has(row.path.toLowerCase()))rows.set(row.path.toLowerCase(),row);
    const restored=[...rows.values()].sort((a,b)=>Date.parse(b.created_at)-Date.parse(a.created_at)).slice(0,12);
    db.prepare('INSERT INTO kv(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value').run('capture_history',JSON.stringify(restored));
    db.exec('COMMIT');
  } catch(error) {db.exec('ROLLBACK');throw error;} finally {db.close();}
}
