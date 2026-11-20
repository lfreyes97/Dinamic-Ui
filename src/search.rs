//! Búsqueda compartida por el command menu (`launcher.rs`) y Actividades
//! (`ui/activities_model.rs`): un único criterio para los dos.
//!
//! Criterio: cada palabra de la consulta debe coincidir con algún campo; la mejor coincidencia
//! de cada palabra puntúa así: prefijo del nombre (100) > inicio de palabra del nombre (80) >
//! contiene en el nombre (60) > `id` (40) > detalle (20). Sin subsecuencia: «frfx» no encuentra
//! «Firefox». Una consulta vacía no puntúa nada: quien llama decide qué mostrar (normalmente, todo).

/// Puntuación de un elemento para la consulta; `None` si alguna palabra no coincide.
///
/// `name` es el título visible, `id` un identificador (vacío si no aplica) y `detail` el texto
/// secundario (comentario, subtítulo, ruta…).
pub fn score(query: &str, name: &str, id: &str, detail: &str) -> Option<i32> {
    let (name, id, detail) = (
        name.to_lowercase(),
        id.to_lowercase(),
        detail.to_lowercase(),
    );
    let mut total = 0;
    let mut any = false;
    for word in query.split_whitespace() {
        any = true;
        let word = word.to_lowercase();
        let score = if name.starts_with(&word) {
            100
        } else if name
            .split([' ', '-', '_', '.'])
            .any(|w| w.starts_with(&word))
        {
            80
        } else if name.contains(&word) {
            60
        } else if id.contains(&word) {
            40
        } else if detail.contains(&word) {
            20
        } else {
            return None;
        };
        total += score;
    }
    any.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_prefers_prefix_then_word_start_then_substring() {
        let prefix = score("fire", "Firefox", "org.mozilla.firefox", "").unwrap();
        let word = score("web", "Epiphany Web", "epiphany", "").unwrap();
        let contains = score("fox", "Firefox", "firefox", "").unwrap();
        let detail = score("navega", "Otra", "otra", "Navega por la web").unwrap();
        assert!(prefix > word && word > contains && contains > detail);
        assert_eq!(score("zzz", "Firefox", "firefox", "web"), None);
        assert_eq!(score("   ", "Firefox", "firefox", ""), None);
        // Con varias palabras deben coincidir todas.
        assert!(score("fire fox", "Firefox", "firefox", "").is_some());
        assert_eq!(score("fire zzz", "Firefox", "firefox", ""), None);
    }

    #[test]
    fn score_matches_id_and_detail_only_as_fallback() {
        // El id cuenta por encima del detalle, pero por debajo del nombre.
        let by_id = score("mozilla", "Firefox", "org.mozilla.firefox", "").unwrap();
        let by_detail = score("mozilla", "Firefox", "firefox", "Mozilla Firefox").unwrap();
        assert!(by_id > by_detail);
        assert!(score("mozilla", "Otra", "", "de mozilla").is_some());
    }

    #[test]
    fn score_is_strict_no_subsequence_and_case_insensitive() {
        // Antes (subsecuencia) «frfx» encontraba «Firefox»; ahora no.
        assert_eq!(score("frfx", "Firefox", "firefox", "Navegador"), None);
        assert_eq!(
            score("FIRE", "firefox", "", ""),
            score("fire", "Firefox", "", "")
        );
        // Las letras sueltas en orden no cuentan como coincidencia.
        assert_eq!(score("xo", "Firefox", "", ""), None);
    }
}
