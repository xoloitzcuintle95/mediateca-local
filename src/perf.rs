// SPDX-License-Identifier: MIT

//! Instrumentación temporal de rendimiento.
//!
//! Todo queda apagado salvo que exista la variable de entorno
//! `MEDIATECA_PERF=1`. Este módulo es temporal: se elimina una vez hecha la
//! medición de escala. Con la variable sin definir, el único costo es una
//! comparación de cadena por punto de medición.

use std::time::Instant;

/// ¿Está activa la instrumentación?
#[must_use]
pub fn enabled() -> bool {
    std::env::var_os("MEDIATECA_PERF").is_some_and(|valor| valor == "1")
}

/// Medición de una fase: imprime `[perf] <ms> ms  <nombre>` al soltarse.
///
/// El guard es `#[must_use]`; para delimitar dónde termina la fase, suéltalo
/// con `drop(timer)`.
#[must_use]
pub struct Timer(&'static str, Instant);

impl Timer {
    /// Inicia la medición; `None` si la instrumentación está apagada.
    #[must_use]
    pub fn start(nombre: &'static str) -> Option<Self> {
        if !enabled() {
            return None;
        }
        Some(Self(nombre, Instant::now()))
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let ms = self.1.elapsed().as_secs_f64() * 1000.0;
        eprintln!("[perf] {ms:.1} ms  {}", self.0);
    }
}

/// Conteo puntual (archivos encontrados, etc.).
pub fn count(nombre: &str, valor: usize) {
    if enabled() {
        eprintln!("[perf] {valor}     {nombre}");
    }
}
