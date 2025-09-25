# `axes`: Guía de Arquitectura y Diseño Técnico

Este documento es una inmersión profunda en el funcionamiento interno de `axes`. Está destinado a desarrolladores que deseen contribuir al proyecto, entender sus decisiones de diseño o simplemente aprender cómo se ha construido una herramienta de CLI compleja y robusta en Rust.

## Principios Arquitectónicos Fundamentales

El diseño de `axes` se basa en un conjunto de principios clave que priorizan la robustez, la mantenibilidad y una experiencia de usuario predecible.

1. **Identidad Inmutable sobre Ubicación Mutable (UUIDs vs. Paths):** Esta es la decisión de diseño más importante. La identidad de un proyecto es su `UUID` (almacenado en `index.bin` y `.axes/project_ref.bin`), no su nombre ni su ruta en el disco. Esto desacopla la organización *lógica* de los proyectos de su organización *física*, permitiendo que operaciones como `rename` y `link` sean seguras y atómicas sin requerir actualizaciones en cascada.

2. **Resolución Perezosa (Lazy Resolution) Controlada por el Handler:** `axes` evita trabajo innecesario. El dispatcher principal es ligero y no resuelve nada. Son los `handlers` de los comandos (`run.rs`, `info.rs`, etc.) los que deciden qué datos necesitan y cuándo cargarlos. La costosa operación de fusionar la configuración de un proyecto (`ResolvedConfig`) solo se realiza si el comando ejecutado la requiere explícitamente.

3. **Handlers Autónomos y API Declarativa:** La interfaz de línea de comandos de `axes` no se define con lógica imperativa, sino de forma declarativa.
    * **Registro de Comandos:** Un `COMMAND_REGISTRY` estático en `axes.rs` es la única fuente de verdad, mapeando nombres de acción y alias a sus funciones handler.
    * **API de Argumentos por Comando:** Cada handler (`info.rs`, `delete.rs`, etc.) define su propia "API" de argumentos a través de una `struct` `clap::Parser` dedicada. Esto encapsula la lógica de parseo y validación de cada comando de forma aislada y robusta.

4. **Separación Estricta de Modelos de Datos:** La manipulación de datos sigue un flujo claro y unidireccional para evitar estados inconsistentes.
    * **`ProjectConfig`:** La representación 1:1 de un archivo `axes.toml`.
    * **`ResolvedConfig`:** La vista completa y fusionada de la configuración de un proyecto, producto de la herencia. Es el modelo con el que operan los handlers.
    * **`ParsedArgs`:** Una estructura intermedia que representa los argumentos de la CLI de forma clasificada (posicional, nombrada) para el motor de expansión.
    * **Sustitutos de Serialización:** Modelos internos (`Serializable...`) se usan para garantizar un formato de caché binario estable y multiplataforma con `bincode`.

5. **Cancelación Cooperativa Global:** Las operaciones de larga duración son completamente cancelables. Un `CancellationToken` global (`Arc<AtomicBool>`), gestionado por `tokio::signal` en `main`, se propaga a través de la aplicación. Los componentes que realizan tareas pesadas (bucles, espera de procesos externos) comprueban este token periódicamente para permitir una terminación segura y ordenada.

---

## Estructura de Módulos del Crate

El código fuente está organizado en módulos con responsabilidades claras y bien definidas.

* `bin/axes.rs`: El punto de entrada de la aplicación. Contiene la función `main` asíncrona con la lógica de `tokio::select!` para la cancelación, y el **dispatcher declarativo** (`COMMAND_REGISTRY` y `run_cli`). Su única misión es enrutar al handler correcto.

* `cli/`: Define la interfaz de usuario de la línea de comandos.
  * `cli.rs`: Define la `struct Cli` principal y "plana" que `clap` usa para el parseo inicial.
  * `handlers/`: **(Corazón de la Lógica de Comandos)**.
    * `mod.rs`: Declara todos los módulos de handlers.
    * `commons.rs`: Funciones de utilidad compartidas por múltiples handlers (ej. `resolve_config_from_context_or_session`, `validate_project_name`).
    * `run.rs`, `init.rs`, etc.: Un módulo por cada acción. Cada uno contiene su `handle` y su `struct` de `Args` para `clap`.

* `core/`: El "cerebro" de la aplicación. No interactúa directamente con el usuario.
  * `config_resolver.rs`: Responsable de tomar un `UUID` y construir la `ResolvedConfig` fusionada.
  * `context_resolver.rs`: Traduce un `String` de contexto del usuario (ej. `mi-app/api/..`) a un `UUID` canónico.
  * `index_manager.rs`: Proporciona la API para leer y escribir en el `index.bin` global.
  * `arg_parser.rs`: El parser que convierte el `Vec<String>` de parámetros de la CLI en la estructura `ParsedArgs`.
  * `interpolator.rs`: El motor de expansión de texto que procesa los tokens `<axes::...>`, colaborando con `arg_parser`.
  * `graph_display.rs`: Lógica de renderizado para el comando `tree`.

* `system/`: Módulos que interactúan con el sistema operativo.
  * `executor.rs`: El motor de ejecución de comandos externos, ahora con **soporte para cancelación**.
  * `shell.rs`: Lógica para el comando `start` y la gestión de sesiones interactivas.

* `models.rs`: Define todas las `struct`s y `enums` de datos del sistema.

* `lib.rs`: El punto de entrada del crate `axes`, donde se declara `CancellationToken`.

## El Ciclo de Vida de un Comando: `axes mi-app/api test --marker smoke`

Para entender cómo colaboran los módulos, sigamos el flujo completo de un comando típico bajo la nueva arquitectura.

1. **`main` y `tokio::select!`:**
    * La aplicación se inicia. `main` crea el `CancellationToken` y lanza dos tareas `async` concurrentes con `tokio::select!`: la escucha de `Ctrl+C` y el `run_cli_wrapper`.

2. **`run_cli` (Dispatcher):**
    * `clap` realiza un parseo inicial y "plano", identificando `context_or_action = "mi-app/api"` y `action_or_context_or_arg = "test"`. El resto (`--marker smoke`) va a `args`.
    * El dispatcher consolida esto. Como no está en modo sesión, aplica la gramática flexible. Ve que `"test"` (un atajo para `run`) no es una acción de sistema, por lo que asume el formato `<contexto> <script> [params...]`.
    * Determina que la acción canónica es `run` y empaqueta un `Vec<String>` de argumentos para su handler: `["mi-app/api", "test", "--marker", "smoke"]`.
    * Busca `"run"` en el `COMMAND_REGISTRY`, encuentra su `handler` (`handlers::run::handle`) y lo invoca, pasándole los argumentos y una referencia al `CancellationToken`.

3. **`handlers/run.rs` (Handler Autónomo):**
    * El `handle` recibe los argumentos. Lo primero que hace es usar su parser de `clap` dedicado, `RunArgs::try_parse_from`, sobre el `Vec<String>`.
    * `clap` puebla la `struct RunArgs` de forma estructurada:
        * `context: "mi-app/api"`
        * `script: "test"`
        * `params: ["--marker", "smoke"]`
    * El handler llama a `commons::resolve_config_from_context_or_session`, pasándole `"mi-app/api"` y el token. Esta es la **primera vez que se accede al índice y se resuelve la configuración**, logrando la resolución perezosa.
    * Se crea el `CommandExecutor`.

4. **`CommandExecutor`, `ArgumentParser` e `Interpolator` (El Motor de Expansión):**
    * `run_script` llama a `execute_internal_script`.
    * `execute_internal_script` obtiene la lista de comandos del script `test` del `axes.toml` (ej. `["pytest <axes::params::marker>"]`).
    * `process_command_list` itera sobre esta lista. Para cada comando:
        a. Se crea una instancia de `ParsedArgs` a partir de `["--marker", "smoke"]`. El `arg_parser` lo clasifica internamente como `named: {"marker": Some("smoke")}`.
        b. Se crea un `Interpolator` pasándole la `ResolvedConfig` y una referencia mutable al `ParsedArgs`.
        c. Se llama a `interpolator.expand_string("pytest <axes::params::marker>", ...)`
        d. El `Interpolator` encuentra el token. Llama a `expand_params_token("marker")`.
        e. `expand_params_token` llama a `parsed_args.consume_named_passthrough("marker")`.
        f. `consume_named_passthrough` encuentra el flag, lo marca como consumido y devuelve el `String` `"--marker smoke"`.
        g. La cadena final se convierte en `"pytest --marker smoke"`.
        h. El `CommandExecutor` comprueba que todos los argumentos (`--marker smoke`) fueron consumidos. Como lo fueron, la validación pasa.

5. **`system/executor.rs` (Ejecución):**
    * El `CommandExecutor` pasa la cadena final `"pytest --marker smoke"` y el `CancellationToken` a `executor::execute_command`.
    * El `executor` inicia el proceso `pytest` en un hilo separado (`spawn`).
    * Entra en un bucle `try_wait()`, comprobando periódicamente si el proceso ha terminado o si el `CancellationToken` ha sido activado.
    * Si el usuario presiona `Ctrl+C`, el token se activa, el `executor` mata el proceso hijo `pytest` y devuelve un error `Cancelled`.
    * Si `pytest` termina, el `executor` devuelve el resultado.

6. **Propagación de Resultados:**
    * El `Result` del `executor` se propaga hacia arriba a través del `CommandExecutor`, el `handler`, `run_cli`, y finalmente a `main`, donde se maneja para la salida final al usuario.

---

## Componentes Clave del Sistema

### El Motor de Expansión

El poder de los scripts de `axes` reside en la colaboración entre dos módulos principales: `arg_parser` y `interpolator`.

#### **`core/arg_parser.rs`**

Este módulo actúa como el motor de "inteligencia de la CLI" para los scripts. Su responsabilidad es tomar el `Vec<String>` de parámetros crudos y convertirlos en una estructura `ParsedArgs` clasificada.

* **Lógica de Parseo:** Implementa un parser simple que distingue entre argumentos **posicionales** y **nombrados (flags)**, y es capaz de asociar valores a los flags (ej. `--target linux`).
* **Gestión de Estado (`consumed`):** Cada argumento parseado tiene un booleano `consumed`. Este estado es la clave que permite al sistema saber qué argumentos han sido utilizados por tokens explícitos (`<axes::params::0>`, `<axes::params::flag=...>`, etc.) y cuáles quedan para ser recogidos por el token genérico `<axes::params>`.

#### **`core/interpolator.rs`**

El `Interpolator` es un motor de expansión de texto recursivo. Su diseño es funcional y sin estado de recursión propio, lo que lo hace robusto y seguro.

* **Lógica de Expansión:** Su función principal, `expand_string_recursive`, itera sobre una cadena de plantilla, buscando tokens `<axes::...>` y reemplazándolos por su valor resuelto.
* **Colaboración con `arg_parser`:** Cuando encuentra un token de parámetro (`<axes::params::...>`), delega la resolución al `ParsedArgs` que se le proporcionó, pidiéndole que consuma y devuelva el valor apropiado.
* **Seguridad:** La lógica de detección de ciclos de scripts (`a -> b -> a`) y la protección contra la recursión infinita (un límite de profundidad máxima) residen aquí, garantizando que las expansiones terminen siempre de forma segura.

### El Ejecutor de Comandos y Sesiones

#### **`system/executor.rs`**

Este módulo es el puente entre `axes` y el sistema operativo. Su diseño prioriza la robustez y la capacidad de interrupción.

* **Ejecución Cancelable:** En lugar de una llamada bloqueante a `status()`, el `executor` usa `spawn()` para obtener un handle del proceso hijo y entra en un bucle `try_wait()`. En cada iteración, comprueba el `CancellationToken` global. Si se ha solicitado una cancelación, `executor` intenta terminar (`kill()`) el proceso hijo antes de salir, previniendo procesos huérfanos.
* **Robustez en Windows:** Mantiene el fallback a `cmd /C` para ejecutar comandos `builtin` del shell de Windows que no son ejecutables independientes.

#### **`system/shell.rs`**

Gestiona la lógica del comando `start`. Su principal responsabilidad es crear un entorno de shell limpio y configurado.

* **Consistencia de Hooks:** Se ha refactorizado para garantizar que tanto el hook `at_start` como `at_exit` sean procesados por el `Interpolator` antes de la ejecución. Esto asegura que los usuarios puedan usar tokens `<axes::...>` en ambos, proporcionando un comportamiento predecible.
* **Scripts Temporales:** Utiliza la creación de scripts temporales (`.bat` o `.sh`) para configurar de forma silenciosa el entorno de la sub-shell (inyectando variables de `[env]` y ejecutando `at_start`) antes de ceder el control al usuario.

### El Sistema de Cancelación (`Ctrl+C`)

La cancelación segura es una característica de primera clase, no una ocurrencia tardía.

* **`tokio::select!` en `main`:** El punto de entrada de la aplicación utiliza `tokio` para ejecutar concurrentemente la lógica principal de `axes` y un listener para la señal de `Ctrl+C`. Esto asegura que la señal de interrupción pueda ser capturada sin bloquear la aplicación y sin interferir con bibliotecas interactivas como `dialoguer`.
* **`CancellationToken` (`Arc<AtomicBool>`):** Cuando se detecta `Ctrl+C`, se actualiza un `AtomicBool` global compartido a través de un `Arc`.
* **Cancelación Cooperativa:** Los componentes de larga duración (el `executor`, los bucles en los handlers `delete`/`unregister`, las búsquedas en el `index_manager`) comprueban el estado de este token en puntos seguros y salen de forma ordenada si se ha solicitado la cancelación, devolviendo un error específico que es manejado limpiamente en `main`.

## El Corazón de `axes`: Modelos de Datos

La robustez de `axes` reside en la clara separación de sus modelos de datos, definidos en `src/models.rs`.

### El Índice Global (`GlobalIndex`)

* **Ubicación:** `~/.config/axes/index.bin`
* **Contenido:** Serializado con `bincode`.
  * `projects: HashMap<Uuid, IndexEntry>`: El grafo principal. Las claves son los `UUID`s de los proyectos, y los `IndexEntry` contienen su nombre, ruta física y el `UUID` de su padre.
  * `aliases: HashMap<String, Uuid>`: Mapa de nombres de alias a los `UUID`s a los que apuntan.
  * `last_used: Option<Uuid>`: El `UUID` del último proyecto resuelto, para el atajo `**`.
* **Propósito:** Es la única fuente de verdad para la **estructura jerárquica** y la **ubicación física** de todos los proyectos. Al ser binario, su carga es casi instantánea.

### La Configuración del Proyecto (`ProjectConfig`)

* **Ubicación:** `<ruta-del-proyecto>/.axes/axes.toml`
* **Contenido:** Una representación 1 a 1 de la estructura del archivo TOML, usando `#[serde(untagged)]` para permitir sintaxis flexibles como secuencias de scripts (`run = "..."` vs `run = ["..."]`).
* **Propósito:** Es el modelo de datos que se lee desde el disco. Es "tonto" y no sabe nada sobre herencia; solo representa el contenido de un único archivo.

### La Configuración Resuelta (`ResolvedConfig`)

* **Ubicación:** Solo en memoria. Es el producto final del `config_resolver`.
* **Contenido:** La `struct` con la que opera la mayor parte de la aplicación. Contiene las secciones `scripts`, `vars`, `env`, etc., ya **fusionadas** desde toda la cadena de herencia. Contiene rutas absolutas y toda la información necesaria para ejecutar una acción.
* **Propósito:** Proporciona una vista completa y autocontenida de la configuración de un proyecto en un momento dado, eliminando la necesidad de que los `handlers` se preocupen por la herencia.

---

## Flujos de Datos Clave

La magia de `axes` reside en cómo transforma una simple entrada del usuario en una configuración completa y ejecutable. Este proceso se divide en dos fases principales, gestionadas por dos resolvedores distintos.

### Resolución de Contexto (`context_resolver`)

El `context_resolver` es la "recepción" de `axes`. Su única responsabilidad es traducir el contexto proporcionado por el usuario (un `String` ambiguo) en una identidad inequívoca (un `Uuid`).

**Algoritmo Principal:**

1. **Análisis de Atajos:** Primero comprueba si el contexto es un atajo de navegación (`.`, `..`, `*`, `**`) o un alias (`mi-api!`). Estos tienen la máxima prioridad.
2. **Resolución de Ruta:** Si no es un atajo, divide el contexto por `/` (ej. `mi-app/api`).
    a. **Primera Parte:** Resuelve el primer segmento. Si coincide con el nombre actual del proyecto raíz, comienza la búsqueda desde la raíz. Si no, asume que es un hijo directo del proyecto raíz.
    b. **Travesía del Grafo:** Por cada segmento restante, busca un hijo del nodo actual con ese nombre.
3. **Resultado:** La función devuelve el `Uuid` final del proyecto objetivo. Es crucial entender que este resolvedor **no lee ningún `axes.toml`**. Opera exclusivamente sobre el `index.bin`, haciéndolo extremadamente rápido.

### Resolución de Configuración (`config_resolver`)

Una vez que un handler tiene un `Uuid`, el `config_resolver` entra en acción para construir su entorno completo.

**Algoritmo Principal:**

1. **Comprobación de Caché:** El primer paso es siempre buscar un `config.cache.bin` válido para el proyecto.
    * **Validación de Dependencias:** El caché almacena los `timestamps` de todos los `axes.toml` en la cadena de herencia. Se comprueba si alguno de los archivos reales en el disco es más reciente que el timestamp guardado. Si es así, el caché está desactualizado y se descarta.
    * Si el caché es válido, se decodifica y se devuelve la `ResolvedConfig` de forma casi instantánea.
2. **Construcción de la Cadena de Herencia:** Si no hay un caché válido, se inicia una travesía ascendente desde el `Uuid` del proyecto objetivo, siguiendo los `parent_uuid` en el `index.bin` hasta llegar al proyecto raíz.
3. **Fusión de Configuraciones:** Se crea una `ResolvedConfig` vacía y se itera sobre la cadena de herencia desde el ancestro más antiguo (raíz) hasta el proyecto objetivo. La configuración de cada nivel se fusiona, con los valores del hijo **sobrescribiendo** los del padre.
4. **Creación de un Nuevo Caché:** La `ResolvedConfig` final, junto con la lista de `axes.toml` dependientes y sus `timestamps`, se serializa a `config.cache.bin`, para que la próxima ejecución sea instantánea.

### El Sistema de Caché y el Patrón de Sustitutos de Serialización

`axes` utiliza `bincode` para una serialización/deserialización binaria ultrarrápida. Para manejar tipos de datos que no son estables entre plataformas (como `PathBuf`), `axes` utiliza el patrón de "sustitutos de serialización":

* Los modelos de trabajo en memoria (`ResolvedConfig`) usan tipos convenientes (`PathBuf`).
* Antes de guardar en caché, se convierten a un "sustituto" (`SerializableResolvedConfig`) que usa tipos estables (`String`).
* Al leer del caché, se decodifica al sustituto y luego se convierte de nuevo al modelo de trabajo.
* Esto aísla completamente la lógica del programa del formato de almacenamiento, combinando rendimiento y robustez.
