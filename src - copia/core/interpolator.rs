// src/core/interpolator.rs

use crate::models::ResolvedConfig;
use dunce;
use std::path::PathBuf;

pub struct Interpolator<'a> {
    config: &'a ResolvedConfig,
    params: &'a [String],
    owner_root: &'a PathBuf,
}

impl<'a> Interpolator<'a> {
    pub fn new(config: &'a ResolvedConfig, params: &'a [String]) -> Self {
        Self {
            config,
            params,
            owner_root: &config.project_root,
        }
    }

    /// Interpola una cadena de texto, reemplazando todos los tokens conocidos
    /// en un orden de precedencia fijo para garantizar la seguridad y la previsibilidad.
    pub fn interpolate(&self, input: &str) -> String {
        let pass1 = self.interpolate_reserved(input);
        let pass2 = self.interpolate_vars(&pass1);
        self.interpolate_params(&pass2)
    }

    /// Reemplaza tokens reservados y metadatos del proyecto.
    fn interpolate_reserved(&self, input: &str) -> String {
        let mut result = input.to_string();

        result = result.replace("{uuid}", &self.config.uuid.to_string());
        result = result.replace("{name}", &self.config.qualified_name);

        // **NUEVA LÓGICA DE FORMATEO DE RUTA**
        // `dunce::canonicalize` hace lo mismo que `std::fs::canonicalize`
        // pero en Windows se asegura de devolver una ruta limpia sin `\\?\`.
        // Sin embargo, como ya tenemos la ruta, solo necesitamos formatearla.
        // Una forma simple es usar dunce para limpiar la ruta que ya tenemos.

        // El `owner_root` también necesita ser limpiado.
        let owner_root_clean = dunce::simplified(self.owner_root).to_string_lossy();
        let current_path_clean = dunce::simplified(&self.config.project_root).to_string_lossy();

        result = result.replace("{root}", &owner_root_clean);
        result = result.replace("{path}", &current_path_clean);

        let version = self.config.version.as_deref().unwrap_or("");
        result = result.replace("{version}", version);

        result
    }

    /// Reemplaza tokens personalizados de la sección [vars] fusionada.
    /// Es importante que esta pasada se ejecute después de `interpolate_reserved`,
    /// para permitir que las variables dependan de los tokens reservados
    /// (ej. `build_dir = "{root}/build"`).
    fn interpolate_vars(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, value) in &self.config.vars {
            let token = format!("{{{}}}", key);
            // También interpolamos el valor de la variable, por si acaso anida otros tokens.
            let interpolated_value = self.interpolate_reserved(value);
            result = result.replace(&token, &interpolated_value);
        }
        result
    }

    /// Reemplaza el token especial `{params}` con los argumentos pasados por el usuario.
    /// Se ejecuta al final para que la entrada del usuario no pueda interferir con
    /// los tokens de configuración.
    fn interpolate_params(&self, input: &str) -> String {
        if input.contains("{params}") {
            let params_str = self.params.join(" ");
            input.replace("{params}", &params_str)
        } else {
            // Si el comando no usa {params}, añadimos los parámetros al final
            // para un comportamiento intuitivo.
            let mut result = input.to_string();
            if !self.params.is_empty() {
                result.push(' ');
                result.push_str(&self.params.join(" "));
            }
            result
        }
    }
}
