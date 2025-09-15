# Hoja de Ruta y Tareas Pendientes (`TODO.md`)

Este documento es la guía de desarrollo para `axes`. Sirve como una hoja de ruta pública de las características planificadas y como un punto de partida para los miembros de la comunidad que deseen contribuir al proyecto.

¡Tu ayuda es bienvenida! Si ves una tarea que te interese, especialmente las marcadas con `[contribución bienvenida]`, no dudes en abrir un "Issue" en GitHub para discutir tu enfoque antes de empezar a trabajar en un "Pull Request".

## Versión Actual: `v0.1.3-alpha`

El estado actual del proyecto es una `alpha` funcional. El núcleo de la gestión de proyectos y la ejecución de comandos es robusto, pero la experiencia de usuario y las características avanzadas todavía están en desarrollo.

---

## Hoja de Ruta Inmediata (Próximas Versiones Alfa)

### v0.1.4: La Experiencia de CLI "Premium"

**Objetivo:** Hacer que la interacción diaria con `axes` en la terminal sea lo más fluida y rápida posible.

- `[ ]` **Implementar Encadenamiento de Comandos (`&&`):**
  - **Descripción:** Permitir al usuario encadenar comandos `run` directamente desde la CLI, como `axes <contexto> run script1 && script2`. Esto permite crear flujos de trabajo complejos sobre la marcha.
  - **Estado:** Pendiente.
- `[ ]` **Implementar Autocompletado de la Shell:** `[contribución bienvenida]`
  - **Descripción:** Integrar `clap_complete` para generar scripts de autocompletado para `bash`, `zsh`, `powershell`, y `fish`. Esto permitirá autocompletar contextos de proyecto, acciones y nombres de scripts.
  - **Estado:** Pendiente.

### v0.1.5: La Interfaz de Usuario Terminal (TUI)

**Objetivo:** Proporcionar una puerta de entrada visual e interactiva al ecosistema de `axes`, ideal para nuevos usuarios y para explorar árboles de proyectos complejos.

- `[ ]` **Implementar el Lanzador de la TUI:**
  - **Descripción:** Cuando se ejecuta `axes` sin argumentos, debe lanzar una interfaz de usuario interactiva basada en texto.
  - **Estado:** Pendiente.
- `[ ]` **Navegación por el Árbol de Proyectos:**
  - **Descripción:** La TUI debe mostrar el árbol de proyectos de forma navegable (con teclas de flecha).
  - **Estado:** Pendiente.
- `[ ]` **Ejecución de Acciones desde la TUI:** `[contribución bienvenida]`
  - **Descripción:** El usuario debe poder seleccionar un proyecto y ver una lista de acciones disponibles (`start`, `info`, `run ...`, etc.) para ejecutarlas directamente desde la TUI.
  - **Estado:** Pendiente.

### v0.1.6: Control de Herencia y Lógica de Scripts Avanzada

**Objetivo:** Dar a los usuarios un control granular sobre la configuración heredada y desbloquear patrones de scripting avanzados.

- `[ ]` **Implementar Herencia Pública/Privada:**
  - **Descripción:** Permitir secciones como `[vars.public]` y `[vars.private]` en el `axes.toml`. El `config_resolver` deberá ser modificado para que solo las secciones públicas se propaguen a los proyectos hijos.
  - **Estado:** Pendiente.
- `[ ]` **Implementar Distinción `{path}` vs `{root}`:**
  - **Descripción:** Modificar el `config_resolver` y el `interpolator` para que, al heredar un comando, se "congele" la ruta de su proyecto de origen. Esto permitirá que `{root}` se refiera al origen del comando y `{path}` al contexto de ejecución actual.
  - **Estado:** Pendiente.

### v0.1.7: El Motor de Andamiaje (`init` 2.0)

**Objetivo:** Transformar `init` de una simple creación de archivos a un motor de plantillas completo.

- `[ ]` **Refactorizar `init` para Usar Plantillas del Sistema de Archivos:**
  - **Descripción:** Modificar `handle_init` para que busque y utilice plantillas (directorios con un `axes.toml.template`) desde `~/.config/axes/templates/`.
  - **Estado:** Pendiente.
- `[ ]` **Implementar Reemplazo de Tokens de Andamiaje (`{{...}}`):**
  - **Descripción:** Implementar la lógica para leer los archivos de una plantilla y reemplazar tokens como `{{name}}`, `{{version}}`, `{{author}}`, etc., con valores proporcionados por el usuario.
  - **Estado:** Pendiente.
- `[ ]` **Implementar `init` Interactivo y con Flags:** `[contribución bienvenida]`
  - **Descripción:** Si se llama a `init` sin argumentos, debe iniciar un asistente que pregunte los valores para los tokens. También debe permitir pasar estos valores a través de flags (ej. `axes init mi-app --template python --set version=1.2.3`).
  - **Estado:** Pendiente.

---

## Hoja de Ruta a Largo Plazo (Post-Alfa / Beta)

Estas son características más ambiciosas que se considerarán una vez que el núcleo del sistema sea estable y haya recibido feedback de la comunidad.

- `[ ]` **Herramienta de Diagnóstico y Reparación (`validate` / `checkout`):**
  - **Descripción:** Un comando `axes validate` que escanee todo el `index.bin` en busca de inconsistencias: enlaces de padre rotos, ciclos, `project_ref.bin` faltantes, rutas que ya no existen, etc.
  - **Futuro:** Añadir un flag `--fix` para intentar reparar automáticamente los problemas encontrados.
- `[ ]` **Cambio de Sesión (`axes change <contexto>`):**
  - **Descripción:** Implementar la capacidad de cambiar de una sesión de proyecto a otra sin salir de la shell principal, utilizando un archivo temporal para la comunicación entre procesos.
- `[ ]` **Gestor de Plantillas Remotas:**
  - **Descripción:** Un comando `axes template install <git-url>` para descargar y configurar nuevas plantillas desde repositorios de Git.
- `[ ]` **Sistema de Idiomas (i18n):** `[contribución bienvenida]`
  - **Descripción:** Internacionalizar los mensajes de la CLI para soportar múltiples idiomas.

---

## ¡Ayúdanos a Probar! (Peticiones de Testing para la v0.1.3-alpha)

¡La mejor forma de contribuir ahora mismo es probando `axes` en tus propios flujos de trabajo! Estamos especialmente interesados en feedback sobre las siguientes áreas:

1. **Robustez del `register`:**
    - Intenta registrar tus monorepos existentes que tengan proyectos anidados.
    - Prueba el escaneo recursivo. ¿Detecta correctamente a tus hijos?
    - Mueve un proyecto registrado a otra ubicación en tu disco y luego intenta registrarlo de nuevo. ¿El asistente interactivo maneja el conflicto de UUID correctamente?

2. **Sintaxis Flexible de la CLI:**
    - Prueba usar `axes <acción> <contexto>` y `axes <contexto> <acción>`. ¿Se comporta como esperas?
    - Crea un proyecto con el mismo nombre que una acción de sistema (ej. `axes init info`). ¿Puedes interactuar con él sin ambigüedades (ej. `axes info info`)?

3. **Comandos `open` y `start` en tu Entorno:**
    - Configura `[options.open_with]` en tu proyecto `global` para tus editores y herramientas favoritas.
    - Prueba la ejecución de `at_start` con la activación de entornos virtuales de diferentes lenguajes (Python `venv`, Node `nvm`, etc.). ¿Funciona como se espera?

4. **Casos Límite de Navegación:**
    - Prueba a componer los contextos de navegación. ¿Funciona `axes mi-app!/../otro-app` como esperas?
    - Usa `*` y `**` en tus flujos de trabajo diarios. ¿Son útiles? ¿Se actualizan correctamente?

**¿Cómo reportar feedback?**
Por favor, abre un "Issue" en nuestro [repositorio de GitHub](https://github.com/RetypeOS/axes/issues), describiendo el problema que encontraste o la sugerencia que tienes. ¡Cualquier feedback es increíblemente valioso en esta etapa!
