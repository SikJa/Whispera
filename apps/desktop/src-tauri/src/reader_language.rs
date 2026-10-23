//! Offline language/voice selection, shared by playback, selection shortcuts and MP3.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use whatlang::{Lang, Script};

pub const AUTO: &str = "auto";
pub type Favorites = BTreeMap<String, String>;
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Voice {
    pub id: String, pub name: String, pub language: String,
    #[serde(default)] pub code: String,
}
#[derive(Clone, Serialize)]
pub struct Analysis { pub kind: String, pub language: Option<String>, pub voice: Option<Voice> }
pub fn canonical(code: &str) -> &str {
    // Windows CultureInfo uses ISO 639-2 macrolanguages for these voices.
    match code { "zho" => "cmn", "fas" => "pes", "nor" => "nob", "fil" => "tgl", _ => code }
}
pub fn validate_favorites(favorites: &Favorites) -> Result<(), String> {
    if favorites.len() > 100 || favorites.iter().any(|(code,id)|code.len()!=3 || !code.bytes().all(|c|c.is_ascii_lowercase()) || id.is_empty() || id.len()>512 || id.chars().any(char::is_control)) {
        return Err("Preferencias de voz por idioma no válidas.".into());
    }
    Ok(())
}
pub fn analyze(text: &str, requested: &str, voices: &[Voice], favorites: &Favorites) -> Analysis {
    let result = |kind: &str, language: Option<String>, voice: Option<Voice>|Analysis{kind:kind.into(),language,voice};
    if requested != AUTO {
        let voice = voices.iter().find(|voice|voice.id==requested).cloned();
        let language = voice.as_ref().map(|voice|canonical(&voice.code).to_owned());
        return result(if voice.is_some(){"manual"}else{"missing_voice"},language,voice);
    }
    if text.trim().is_empty() {return result("empty",None,None);}
    if voices.is_empty() {return result("no_voices",None,None);}
    let Some(info) = whatlang::detect(text) else {return result("uncertain",None,None);};
    // Do not guess from names, one/two Latin words, punctuation or tiny samples.
    let letters = text.chars().filter(|c|c.is_alphabetic()).count();
    let words = text.split(|c:char|!c.is_alphabetic()).filter(|word|!word.is_empty()).count();
    if letters<12 || (info.script()==Script::Latin && words<3) || !info.is_reliable() {
        return result("uncertain",None,None);
    }
    let language = info.lang().code();
    // Detection is never restricted to installed languages: missing French must
    // not be classified as English merely because only English is installed.
    let compatible = |voice: &&Voice|canonical(&voice.code)==language;
    let voice = favorites.get(language).and_then(|id|voices.iter().filter(compatible).find(|voice|&voice.id==id))
        .or_else(||voices.iter().find(compatible)).cloned();
    result(if voice.is_some(){"matched"}else{"missing_voice"},Some(language.into()),voice)
}
impl Analysis {
    pub fn require_voice(self) -> Result<Voice, String> {
        if let Some(voice) = self.voice {return Ok(voice);}
        Err(match self.kind.as_str() {
            "uncertain"|"empty"=>"No pude identificar el idioma con suficiente seguridad. Elegí una voz manual en Lectura.".into(),
            "no_voices"=>"No hay voces de escritorio instaladas.".into(),
            _=>match self.language.and_then(|code|Lang::from_code(&code)) {
                Some(lang)=>format!("No hay una voz instalada para {}. Instalá una compatible o elegí una voz manual en Lectura.",lang.name()),
                None=>"La voz elegida no está instalada. Elegí otra voz en Lectura.".into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn voice(id:&str,code:&str)->Voice{Voice{id:id.into(),code:code.into(),..Voice::default()}}
    const ES:&str="Este documento explica cómo preparar una lectura en español. Podemos escuchar las instrucciones con tranquilidad y guardar el resultado para consultarlo más adelante.";
    const EN:&str="This document explains how to prepare a reading in English. You can listen to the instructions carefully and save the result to review it later.";
    const FR:&str="Ce document explique comment préparer une lecture en français. Vous pouvez écouter les instructions tranquillement et enregistrer le résultat pour le consulter plus tard.";
    #[test] fn reader_language_selects_text_language_and_preferred_accent() {
        let voices=vec![voice("English","eng"),voice("España","spa"),voice("México","spa")];
        let favorites=Favorites::from([("spa".into(),"México".into())]);
        assert_eq!(analyze(ES,AUTO,&voices,&favorites).require_voice().unwrap().id,"México");
        assert_eq!(analyze(EN,AUTO,&voices,&favorites).require_voice().unwrap().id,"English");
        let mixed=format!("{ES}\n{ES}\n{EN}\n{ES}");
        assert_eq!(analyze(&mixed,AUTO,&voices,&favorites).require_voice().unwrap().id,"México");
        let absent=Favorites::from([("spa".into(),"removed voice".into())]);
        assert_eq!(analyze(ES,AUTO,&voices,&absent).require_voice().unwrap().id,"España");
    }
    #[test] fn reader_language_never_substitutes_an_unrelated_language() {
        let voices=vec![voice("English","eng"),voice("Español","spa")];
        let result=analyze(FR,AUTO,&voices,&Favorites::new());
        assert_eq!(result.language.as_deref(),Some("fra"));assert_eq!(result.kind,"missing_voice");assert!(result.require_voice().is_err());
        assert_eq!(analyze(FR,"English",&voices,&Favorites::new()).require_voice().unwrap().id,"English");
        assert!(analyze(ES,"removed manual voice",&voices,&Favorites::new()).require_voice().is_err());
        let chinese=vec![voice("Mandarin","zho")];
        assert_eq!(analyze("这是一段中文朗读测试，我们希望使用已经安装的中文语音来阅读这个文档。",AUTO,&chinese,&Favorites::new()).require_voice().unwrap().id,"Mandarin");
    }
    #[test] fn reader_language_ambiguous_input_requires_manual_choice() {
        let voices=vec![voice("English","eng")];
        for text in ["Hola","Hello world","12345 😀 !!!","Daniel Microsoft"] {
            assert_eq!(analyze(text,AUTO,&voices,&Favorites::new()).kind,"uncertain");
            assert_eq!(analyze(text,"English",&voices,&Favorites::new()).require_voice().unwrap().id,"English");
        }
        assert_eq!(analyze("",AUTO,&voices,&Favorites::new()).kind,"empty");
    }
    #[test] fn reader_language_windows_codes_and_preferences_are_bounded() {
        assert_eq!(canonical("zho"),"cmn");assert_eq!(canonical("fas"),"pes");
        let good=Favorites::from([("spa".into(),"Installed voice".into())]);assert!(validate_favorites(&good).is_ok());
        assert!(validate_favorites(&Favorites::from([("en-US".into(),"voice".into())])).is_err());
        assert!(validate_favorites(&Favorites::from([("eng".into(),"bad\0voice".into())])).is_err());
    }
}
