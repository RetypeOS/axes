Excelente ajuste. Priorizar `arg2` como la acción (`axes <contexto> <acción>`) sobre `arg1` es más intuitivo y se alinea con la sintaxis natural de la mayoría de las herramientas de CLI. Es el refinamiento correcto.

Ahora, abordemos la siguiente fase: analizar cada acción bajo el nuevo paradigma de "handlers responsables" y determinar cuáles pueden operar globalmente (sin contexto) y cómo podemos optimizar su funcionamiento interno.

---

### **Análisis y Propuestas de Refactorización por Acción**

A continuación, presento una lista de todas las acciones del sistema, evaluando su comportamiento con y sin contexto, y proponiendo optimizaciones o mejoras.

#### **1. `init`**

*   **Comportamiento Actual:** `axes init <nombre> [--parent <contexto>]`.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> init`):** No tiene sentido semántico. Un proyecto se inicializa en el directorio actual, no "en" otro proyecto lógico.
    *   **Sin Contexto (`axes init [nombre]`):** Este es su modo de operación natural.
*   **Propuesta de Refactorización:**
    *   El handler `handle_init` ignorará por completo el `context: Option<String>` que reciba. Su lógica se basará únicamente en los `args` (el nombre y los flags como `--parent`).
    *   **Optimización:** La lógica actual ya es bastante sólida. Sin embargo, podríamos mejorar la experiencia de usuario: si no se proporciona un nombre en los `args`, en lugar de fallar, `handle_init` podría iniciar un modo interactivo que pregunte el nombre del proyecto y el padre (si lo desea), reutilizando la lógica de `dialoguer` que ya usamos en otras partes. Esto se alinea con la futura hoja de ruta (`init 2.0`).

#### **2. `register`**

*   **Comportamiento Actual:** `axes register [ruta] [--autosolve]`.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> register`):** Similar a `init`, no tiene mucho sentido. Registrar un proyecto es una acción sobre el sistema de archivos.
    *   **Sin Contexto (`axes register [ruta]`):** Es su modo de operación principal. El primer argumento posicional (si existe) se interpreta como una ruta.
*   **Propuesta de Refactorización:**
    *   El handler `handle_register` también ignorará el `context`. Su lógica para parsear la ruta y los flags desde `args` ya es robusta y debe mantenerse.
    *   **Optimización:** No requiere una optimización significativa en este momento. La máquina de estados en `onboarding_manager` ya es bastante compleja y eficiente.

#### **3. `alias`**

*   **Comportamiento Actual:** `axes alias <set|list|rm> [args...]`.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> alias ...`):** No tiene sentido. Los alias son un mecanismo global que apunta a proyectos; no pertenecen a un contexto específico.
    *   **Sin Contexto (`axes alias ...`):** Su único modo de operación.
*   **Propuesta de Refactorización:**
    *   El handler `handle_alias` ignorará el `context`. La lógica actual que parsea el subcomando y los argumentos desde `args` es correcta.
    *   **Optimización:** No requiere cambios.

#### **4. `tree`**

*   **Comportamiento Actual:** `axes <ctx> tree` o `axes tree <ctx>`. Muestra el subárbol. `axes tree` (sin contexto) muestra el árbol global.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> tree`):** Comportamiento útil y deseado: mostrar el subárbol a partir de `<ctx>`.
    *   **Sin Contexto (`axes tree`):** Comportamiento útil y deseado: mostrar el árbol completo.
*   **Propuesta de Refactorización:**
    *   El handler `handle_tree(context: Option<String>)` ya está correctamente implementado. Si `context` es `Some`, resuelve el UUID y lo usa como nodo de inicio. Si es `None`, muestra el árbol completo.
    *   **Optimización:** El rendimiento actual depende de la carga del `GlobalIndex`, que ya es binario y rápido. La construcción del `children_map` en `graph_display` es eficiente. No se necesita optimización por ahora.

#### **5. `start` y `info`**

*   **Comportamiento Actual:** `axes <ctx> start`, `axes <ctx> info`.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> start|info`):** Su único modo de operación válido.
    *   **Sin Contexto (`axes start|info`):** No tiene sentido. ¿En qué proyecto se iniciaría la sesión o de cuál se mostraría la información?
*   **Propuesta de Refactorización:**
    *   Los handlers `handle_start` y `handle_info` deben usar la función auxiliar `resolve_project_config(context)`. Si el `context` es `None`, esta función ya devuelve un error claro (`Esta acción requiere un contexto de proyecto.`), lo cual es el comportamiento correcto.
    *   **Optimización:** Ambas acciones dependen de `config_resolver`, que ya tiene un sistema de caché robusto. No hay optimizaciones obvias o necesarias aquí.

#### **6. `run` y Atajos de Script (`axes <ctx> <script>`)**

*   **Comportamiento Actual:** `axes <ctx> run <script> [params...]` o el atajo.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto (`axes <ctx> run ...`):** Su único modo de operación válido.
    *   **Sin Contexto (`axes run ...`):** No tiene sentido. ¿Dónde se buscaría el script a ejecutar?
*   **Propuesta de Refactorización:**
    *   `handle_run` debe, al igual que `start` e `info`, exigir un contexto a través de `resolve_project_config`.
    *   **Optimización (A Futuro):** Podríamos considerar permitir la ejecución de scripts del proyecto `global` sin especificarlo explícitamente (ej: `axes deploy-docs` si `deploy-docs` está en `global`). Sin embargo, esto podría crear ambigüedad si un proyecto también se llama `deploy-docs`. Por ahora, la regla explícita (`axes global deploy-docs`) es más segura y clara. No implementaremos cambios aquí todavía.

#### **7. `open`, `rename`, `link`, `unregister`, `delete`**

*   **Comportamiento Actual:** Requieren un contexto de proyecto.
*   **Análisis bajo el Nuevo Paradigma:**
    *   **Con Contexto:** Su único modo de operación válido. No se puede renombrar, enlazar o borrar "nada".
    *   **Sin Contexto:** No tienen sentido.
*   **Propuesta de Refactorización:**
    *   Todos estos handlers deben exigir un contexto a través de `resolve_project_config`.
    *   **Optimización:** Estas son operaciones de escritura sobre el `GlobalIndex`. La principal "optimización" es asegurar que las validaciones (anti-ciclos, colisiones de nombres) dentro de `index_manager` sean lo más eficientes posible, lo cual ya parecen ser. La invalidación de cachés es implícita (la próxima resolución fallará al encontrar el caché y lo regenerará), lo cual es correcto.

### **Tabla Resumen de Acciones y Comportamiento**

| Acción       | Admite Contexto Global (Sin Contexto Explícito) | Comportamiento sin Contexto                                  | Comportamiento con Contexto                          | Propuesta de Optimización/Mejora                                                                                                  |
| :----------- | :---------------------------------------------: | :----------------------------------------------------------- | :--------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------- |
| `init`       |                       ✅                        | **Operación principal.** Crea un proyecto en el `cwd`.         | Ignorado.                                            | Hacer el modo interactivo (preguntar nombre/padre) si no se proporcionan `args`.                                                 |
| `register`   |                       ✅                        | **Operación principal.** Registra un proyecto en el `cwd` o ruta. | Ignorado.                                            | Ninguna por ahora.                                                                                                                |
| `alias`      |                       ✅                        | **Operación principal.** Gestiona los alias globales.          | Ignorado.                                            | Ninguna por ahora.                                                                                                                |
| `tree`       |                       ✅                        | Muestra el árbol de proyectos completo.                      | Muestra el subárbol a partir del contexto.           | Ninguna por ahora.                                                                                                                |
| `start`      |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Inicia sesión en el proyecto. | Ninguna.                                                                                                                          |
| `info`       |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Muestra info del proyecto.    | Ninguna.                                                                                                                          |
| `run`        |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Ejecuta un script.            | Considerar (a futuro) un fallback a scripts `global` si el contexto es ambiguo.                                                  |
| `open`       |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Abre el proyecto.             | Ninguna.                                                                                                                          |
| `rename`     |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Renombra el proyecto.         | Ninguna.                                                                                                                          |
| `link`       |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Mueve el proyecto.            | Ninguna.                                                                                                                          |
| `unregister` |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Desregistra el proyecto.      | Ninguna.                                                                                                                          |
| `delete`     |                       ❌                        | Error: "Contexto requerido".                                 | **Operación principal.** Borra el proyecto.            | Ninguna.                                                                                                                          |

Este análisis nos da una hoja de ruta clara para refactorizar cada handler. Las acciones se dividen limpiamente en dos grupos:

1.  **Acciones Globales/Autocontenidas (`init`, `register`, `alias`, `tree`):** Pueden o deben operar sin un contexto de proyecto específico. Sus handlers estarán preparados para `context: Option<String>` y actuarán en consecuencia.
2.  **Acciones Dependientes de Contexto (el resto):** Requieren un proyecto sobre el cual actuar. Sus handlers usarán `resolve_project_config` y fallarán si no se proporciona un contexto válido.

El siguiente paso sería aplicar estas lógicas a cada uno de los handlers que aún no han sido completamente adaptados. ¿Procedemos?