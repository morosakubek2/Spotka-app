use std::collections::HashMap;
use std::sync::RwLock;
use serde_json::Value;
use once_cell::sync::Lazy;

static TRANSLATIONS: Lazy<RwLock<HashMap<String, HashMap<String, String>>>> = Lazy::new(|| {
    RwLock::new(HashMap::new())
});

pub fn load_language(lang_code: &str, json_content: &str) -> Result<(), String> {
    let map: HashMap<String, String> = serde_json::from_str(json_content)
        .map_err(|e| format!("Błąd parsowania JSON: {}", e))?;
    
    let mut translations = TRANSLATIONS.write().unwrap();
    translations.insert(lang_code.to_string(), map);
    Ok(())
}

pub fn tr(key: &str) -> String {
    // Domyślnie angielski lub pierwszy dostępny
    let translations = TRANSLATIONS.read().unwrap();
    
    // Próba pobrania z aktualnego języka (tu uproszczone: sprawdzamy 'en', potem 'pl')
    // W pełnej wersji trzeba by trzymać current_lang w stanie globalnym
    if let Some(en_map) = translations.get("en") {
        if let Some(val) = en_map.get(key) {
            return val.clone();
        }
    }
    if let Some(pl_map) = translations.get("pl") {
        if let Some(val) = pl_map.get(key) {
            return val.clone();
        }
    }
    
    // Fallback: zwróć klucz jeśli brak tłumaczenia
    key.to_string()
}

pub fn set_current_lang(_code: &str) {
    // Tu powinna być logika zmiany aktywnego języka w stanie globalnym
    // Dla potrzeb Slint, funkcja tr() będzie wywoływana z kontekstem
}
