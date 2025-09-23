# axes: Guía de Arquitectura y Diseño Técnico

Este documento es una inmersión profunda en el funcionamiento interno de `axes`. Está destinado a desarrolladores que deseen contribuir al proyecto, entender sus decisiones de diseño o simplemente aprender cómo se ha construido una herramienta de CLI compleja en Rust.

## Índice

- [Principios Arquitectónicos Fundamentales](#principios-arquitectónicos-fundamentales)
- [Estructura de Módulos del Crate](#estructura-de-módulos-del-crate)
- [El Ciclo de Vida de un Comando: Un Análisis Detallado](#el-ciclo-de-vida-de-un-comando-un-análisis-detallado)
- [El Corazón de `axes`: Modelos de Datos](#el-corazón-de-axes-modelos-de-datos)
  - [El Índice Global (`GlobalIndex`)](#el-índice-global-globalindex)
  - [La Configuración del Proyecto (`ProjectConfig`)](#la-configuración-del-proyecto-projectconfig)
  - [La Configuración Resuelta (`ResolvedConfig`)](#la-configuración-resuelta-resolvedconfig)
- [Flujos de Datos Clave](#flujos-de-datos-clave)
  - [Resolución de Contexto (`context_resolver`)](#resolución-de-contexto-context_resolver)
  - [Resolución de Configuración (`config_resolver`)](#resolución-de-configuración-config_resolver)
  - [El Sistema de Caché](#el-sistema-de-caché)
- [El Ejecutor de Comandos y Shells](#el-ejecutor-de-comandos-y-shells)
- [La Máquina de Estados de Onboarding (`onboarding_manager`)](#la-máquina-de-estados-de-onboarding-onboarding_manager)

---

## Principios Arquitectónicos Fundamentales

El diseño de `axes` se basa en varias decisiones clave que dictan su comportamiento y estructura.

1. **El Grafo de Proyectos es la Única Fuente de Verdad:** `axes` no opera sobre el sistema de archivos directamente para la navegación. Toda la estructura jerárquica de los proyectos (quién es padre de quién) vive exclusivamente en el archivo `index.bin`. Esto desacopla la organización lógica de los proyectos de su organización física en el disco.

2. **Identidad Inmutable (UUIDs):** La identidad de un proyecto es su UUID, no su nombre ni su ruta. Esto permite que operaciones como `rename` y `link` sean seguras y no requieran actualizaciones en cascada de los descendientes, garantizando la integridad del grafo.

3. **Resolución Perezosa y con Múltiples Capas de Caché:** Recorrer el árbol de herencia puede ser costoso. Para que las ejecuciones repetidas sean instantáneas, `axes` utiliza un sistema de caché agresivo:
    - **`index.bin`:** El propio índice es binario para una carga inicial ultrarrápida.
    - **Cachés locales (`.axes/*.bin`):** Cada proyecto puede cachear información como sus hijos directos o su último hijo usado, para acelerar la navegación por el árbol.
    - **Caché de Configuración Resuelta (`config.cache.bin`):** El resultado final de la fusión de toda la cadena de herencia se guarda localmente. Este caché se invalida automáticamente si cualquier `axes.toml` en la cadena de herencia es modificado.

4. **Separación Estricta entre Lógica y Presentación:** El núcleo de `axes` (`core/`) opera con datos estructurados y UUIDs. La capa de la CLI (`bin/axes.rs`) y sus módulos (`cli/`) se encargan de traducir la entrada del usuario (strings, alias) a estas estructuras y de formatear la salida.

## Estructura de Módulos del Crate

El código fuente está organizado en módulos con responsabilidades claras:

- `bin/axes.rs`: El punto de entrada de la aplicación. Contiene la función `main` y el **despachador de la CLI (`run_cli`)**. Su trabajo es parsear los argumentos iniciales y delegar el control al `handle` o módulo apropiado.
- `cli.rs`: Define la estructura de la línea de comandos usando la crate `clap`. Documenta la interfaz pública de la herramienta.
- `constants.rs`: Centraliza todos los nombres de archivos y directorios (ej. `.axes`, `index.bin`) para evitar strings mágicos y facilitar cambios.
- `models.rs`: **El módulo más importante.** Define todas las `structs` y `enums` que representan los datos del sistema, desde la configuración en disco (`ProjectConfig`) hasta los modelos en memoria (`ResolvedConfig`) y los sustitutos de serialización para el caché.
- `core/`: El "cerebro" de la aplicación. No interactúa directamente con el usuario.
  - `config_resolver.rs`: Toma un `UUID` y una cadena de herencia, y produce la `ResolvedConfig` final fusionada. Contiene la lógica de herencia y el manejo del caché de configuración.
  - `context_resolver.rs`: Toma un `String` del usuario (ej. `mi-app!/api/..`) y lo traduce a un `UUID` canónico. Contiene la lógica de navegación (alias, `*`, `.` , `_`, `..`).
  - `index_manager.rs`: Proporciona una API segura para leer y modificar el `index.bin` global y los archivos `project_ref.bin` locales.
  - `onboarding_manager.rs`: Contiene la "máquina de estados" para `register`, guiando al usuario para integrar proyectos existentes.
  - `interpolator.rs`: Maneja el reemplazo de tokens (`{...}`) en las cadenas de comandos.
  - `graph_display.rs`: Contiene la lógica para renderizar el árbol de proyectos en formato ASCII.
- `system/`: Módulos que interactúan con el sistema operativo.
  - `executor.rs`: El motor de ejecución de comandos. Utiliza `shlex` para un parseo robusto y un enfoque de "probar directo primero, con fallback a shell" para manejar tanto ejecutables como comandos internos de `cmd.exe`.
  - `shell.rs`: Contiene la lógica para el comando `start`, incluyendo la creación de scripts temporales para una configuración silenciosa del entorno y el manejo de `at_start` y `at_exit`.

## El Ciclo de Vida de un Comando: Un Análisis Detallado

Veamos el flujo completo de un comando como `axes mi-app/api check`:

1. **`main` -> `run_cli`:** Se parsean los argumentos. `context_or_action` es `"mi-app/api"`, `action_or_context_or_arg` es `"check"`.
2. **`run_cli` -> `determine_context_and_action`:** El despachador determina que `"mi-app/api"` no es una acción de sistema, pero `"check"` podría serlo. Como no lo es, asume el atajo para `run`. Devuelve:
    - `context_str = "mi-app/api"`
    - `action_str = "run"`
    - `action_args = ["check"]`
3. **`run_cli` -> `dispatch_action`:** La acción `"run"` no es global, por lo que se procede a la resolución.
4. **`dispatch_action` -> `context_resolver::resolve_context`:**
    - Se recibe la cadena `"mi-app/api"`.
    - No es un alias. Se divide en `["mi-app", "api"]`.
    - `resolve_first_part` busca en el `index.bin` un hijo de `global` llamado `"mi-app"`. Encuentra su UUID (ej. `uuid-A`).
    - El bucle de travesía busca un hijo de `uuid-A` llamado `"api"`. Encuentra su UUID (ej. `uuid-B`).
    - Se llama a `build_qualified_name` con `uuid-B` para reconstruir la ruta canónica, que podría ser `global/mi-app/api`.
    - Se actualizan los cachés de "último usado".
    - Se devuelve `(uuid-B, "global/mi-app/api")`.
5. **`dispatch_action` -> `config_resolver::resolve_config_for_uuid`:**
    - Recibe `uuid-B` y su nombre.
    - Busca un `config.cache.bin` en la ruta de `api`. Si es válido y sus dependencias (timestamps de `global.toml`, `mi-app.toml`, `api.toml`) no han cambiado, devuelve la `ResolvedConfig` cacheada y el proceso salta al paso 7.
    - Si el caché no es válido:
        - Construye la cadena de herencia ascendente: `[global_entry, mi-app_entry, api_entry]`.
        - Lee cada `axes.toml` correspondiente.
        - Fusiona las configuraciones en orden (padre -> hijo), sobrescribiendo valores.
        - Crea la `ResolvedConfig` final.
        - Escribe esta `ResolvedConfig` en un nuevo `config.cache.bin`.
6. **`dispatch_action` -> `execute_project_action` -> `handle_run`:**
    - `handle_run` recibe la `ResolvedConfig` completa.
    - Encuentra el comando `check` en `config.scripts`.
    - Si es una secuencia `["cargo check", "echo OK"]`, entra en un bucle.
    - Para cada comando de la secuencia:
        - Llama al **`interpolator`** para reemplazar `{name}`, `{path}`, etc.
        - Pasa el comando interpolado y el `config.env` al **`executor`**.
7. **`executor::execute_command`:**
    - Usa `shlex` para parsear el comando.
    - Intenta ejecutarlo directamente.
    - Si falla con `NotFound` en Windows, reintenta con `cmd /C`.
    - Espera a que el proceso termine y devuelve el resultado.
8. **`handle_run`:** Si algún comando de la secuencia falla, el `?` propaga el error hacia arriba, deteniendo la ejecución. Si todos tienen éxito, termina.

## El Corazón de `axes`: Modelos de Datos

La robustez de `axes` reside en la clara separación de sus modelos de datos.

### El Índice Global (`GlobalIndex`)

- **Ubicación:** `~/.config/axes/index.bin`
- **Contenido:**
  - `projects: HashMap<Uuid, IndexEntry>`: El grafo principal. Las claves son los UUIDs de los proyectos.
  - `aliases: HashMap<String, Uuid>`: Mapa de nombres de alias a los UUIDs a los que apuntan.
  - `last_used: Option<Uuid>`: El UUID del último proyecto resuelto, para el atajo `**`.
- **Propósito:** Es la única fuente de verdad para la estructura y ubicación de todos los proyectos. Al ser binario, su carga es casi instantánea.

### La Configuración del Proyecto (`ProjectConfig`)

- **Ubicación:** `<ruta-del-proyecto>/.axes/axes.toml`
- **Contenido:** Una representación 1 a 1 de la estructura del archivo TOML, usando `#[serde(untagged)]` para permitir sintaxis flexibles como secuencias de comandos.
- **Propósito:** Es el modelo de datos que se lee desde el disco. Es "tonto" y no sabe nada sobre herencia; solo representa el contenido de un único archivo.

### La Configuración Resuelta (`ResolvedConfig`)

- **Ubicación:** Solo en memoria. Es el producto final del `config_resolver`.
- **Contenido:** La `struct` con la que opera la mayor parte de la aplicación. Contiene las secciones `scripts`, `vars`, `env`, etc., ya fusionadas desde toda la cadena de herencia. Contiene rutas absolutas y toda la información necesaria para ejecutar una acción.
- **Propósito:** Proporciona una vista completa y autocontenida de la configuración de un proyecto en un momento dado, eliminando la necesidad de que los `handles` (`handle_run`, `handle_open`) se preocupen por la herencia.

## Flujos de Datos Clave

La magia de `axes` reside en cómo transforma una simple entrada del usuario en una configuración completa y ejecutable. Este proceso se divide en dos fases principales, gestionadas por dos resolvedores distintos.

### Resolución de Contexto (`context_resolver`)

El `context_resolver` es la "recepción" de `axes`. Su única responsabilidad es traducir el contexto proporcionado por el usuario (un `String` ambiguo) en una identidad inequívoca (un `Uuid`).

**Algoritmo Principal:**

1. **Análisis de Prefijo/Sufijo:** El resolvedor primero comprueba si el contexto es un alias (ej. `api!`). Si lo es, busca en el `index.bin`, expande el alias a su `Uuid` canónico, y su trabajo ha terminado.
2. **División de Ruta:** Si no es un alias, divide el contexto por `/` para obtener una secuencia de "saltos" de navegación (ej. `mi-app/api` -> `["mi-app", "api"]`).
3. **Resolución del Primer Salto:** La primera parte de la ruta tiene reglas especiales:
    - `**`: Se traduce al `Uuid` guardado en `index.last_used`.
    - `.` o `_`: Se invoca a `find_project_from_path`, que analiza el `index.bin` en busca de proyectos cuya ruta física contenga o coincida con el directorio de trabajo actual.
    - `global`: Se traduce al `Uuid` constante de `global` (`Uuid::nil()`).
    - `nombre`: Se asume que es una ruta implícita y se busca un hijo directo del proyecto `global` con ese nombre.
4. **Travesía del Grafo:** Por cada "salto" restante en la ruta:
    - `..`: Se mueve al `parent_uuid` del nodo actual.
    - `*`: Se lee el `last_used.cache.bin` del nodo actual para encontrar el último hijo visitado. Si no existe, se inicia un fallback interactivo.
    - `nombre`: Se busca un hijo del nodo actual con ese nombre.
5. **Resultado:** La función devuelve el `Uuid` final del proyecto objetivo. Es crucial entender que este resolvedor **no lee ningún `axes.toml`**. Opera exclusivamente sobre el `index.bin` y los cachés de navegación, haciéndolo extremadamente rápido.

### Resolución de Configuración (`config_resolver`)

Una vez que tenemos un `Uuid`, el `config_resolver` entra en acción para construir su entorno completo.

**Algoritmo Principal:**

1. **Comprobación de Caché de Configuración:** El primer paso es siempre comprobar si existe un `config.cache.bin` válido para el proyecto.
    - **Validación de Formato:** Se intenta decodificar el archivo. Si falla (ej. por un `UnexpectedEnd`), el caché se considera corrupto, se elimina automáticamente, y se procede a la resolución completa.
    - **Validación de Dependencias:** El caché almacena los timestamps de todos los `axes.toml` en la cadena de herencia. Se comprueba si alguno de los archivos reales en el disco es más reciente que el timestamp guardado. Si es así, el caché está desactualizado y se descarta.
    - Si el caché es válido, se devuelve la `ResolvedConfig` y el proceso termina aquí.
2. **Construcción de la Cadena de Herencia:** Si no hay un caché válido, se inicia una travesía ascendente desde el `Uuid` del proyecto objetivo. Siguiendo los `parent_uuid` en el `index.bin`, se construye una lista de todos los ancestros, hasta llegar a `global`.
3. **Fusión de Configuraciones:**
    - Se crea una `ResolvedConfig` vacía.
    - Se itera sobre la cadena de herencia en orden, desde el ancestro más antiguo (`global`) hasta el proyecto objetivo.
    - Para cada `ProjectConfig` en la cadena, sus `HashMap`s (`vars`, `env`, `options.open_with`, etc.) se fusionan en la `ResolvedConfig`. El método `extend()` de `HashMap` asegura que las claves del hijo sobrescriban las del padre.
    - Los campos `Option<String>` (como `version`) se fusionan con una lógica de `hijo.or(padre)`.
4. **Creación de un Nuevo Caché:** La `ResolvedConfig` final, junto con la lista de `axes.toml` dependientes y sus timestamps, se serializa a `config.cache.bin`, para que la próxima ejecución sea instantánea.

### El Sistema de Caché

`axes` utiliza un sistema de caché binario en múltiples niveles, basado en la crate `bincode`.

**El Patrón de Sustitutos de Serialización:**

El problema principal con `bincode` es que se niega a serializar tipos de datos que no tienen una representación binaria estable y multiplataforma, como `PathBuf` (diferente en Windows/Linux) o `enum`s con `#[serde(untagged)]`.

Para resolver esto, `axes` utiliza el patrón de "sustitutos de serialización":

1. **Modelos de Trabajo (en memoria):** `structs` como `ResolvedConfig` usan tipos convenientes para el programa (`PathBuf`, `enum Command`). Estas `structs` **no** derivan `Serialize` o `Deserialize`.
2. **Modelos de Serialización (para disco):** `structs` privadas como `SerializableResolvedConfig` son un reflejo de las de trabajo, pero reemplazan los tipos problemáticos con sustitutos estables:
    - `PathBuf` -> `String`
    - `enum Command` (untagged) -> `enum SerializableCommand` (tagged)
    - `SystemTime` -> `SerializableSystemTime` (un wrapper sobre `Duration`)
3. **Capa de Conversión (`impl From`):** Se implementa una lógica de conversión `From`/`Into` entre los modelos de trabajo y los de serialización.
4. **Flujo:** Al escribir un caché, `axes` convierte la `struct` de trabajo a su sustituto serializable antes de llamar a `bincode`. Al leer, decodifica al sustituto y luego lo convierte de vuelta a la `struct` de trabajo. Esto aísla completamente la lógica del programa del formato de almacenamiento.

## El Ejecutor de Comandos y Shells

`axes` distingue entre la ejecución de un comando no interactivo (`run`) y el lanzamiento de una shell interactiva (`start`).

### El Ejecutor de `run` (`executor.rs`)

El objetivo es ejecutar un comando de la forma más predecible y menos "mágica" posible.

1. **Parseo con `shlex`:** La cadena de comando se parsea con `shlex::split`, que entiende las comillas y los espacios, dividiendo la cadena en un "programa" y una lista de "argumentos". Esto evita los problemas de doble escapado de comillas.
2. **Enfoque "Probar Directo Primero":**
    - `axes` intenta ejecutar el "programa" directamente, pasándole los "argumentos". Esto funciona para el 99% de los casos (`cargo`, `git`, `code`, `explorer.exe`).
    - **Fallback para `builtins`:** Si la ejecución directa falla con un error específico (`ErrorKind::NotFound`) **y** estamos en Windows, `axes` asume que podría ser un comando interno del shell (como `echo`, `cd`, `start`). Solo en este caso, reintenta la ejecución envolviendo la cadena original completa en `cmd /C "..."`.
3. **Manejo de Errores de Salida (`-`):** Si la cadena de comando original comienza con un guion (`-`), el ejecutor ignorará cualquier código de salida no nulo, tratando la ejecución como un éxito. Esto es esencial para aplicaciones GUI que no siguen las convenciones de la CLI.
4. **Limpieza de Rutas (`dunce`):** Antes de establecer el directorio de trabajo (CWD) para el nuevo proceso, la ruta se limpia con la crate `dunce` para eliminar el prefijo `\\?\` en Windows, que no es entendido por `cmd.exe`.

#### El Lanzador de `start` (`shell.rs`)

El objetivo es crear un entorno de shell limpio y configurado sin que el usuario vea los comandos de preparación.

1. **Configuración Externa (`shells.toml`):** La información sobre cómo lanzar cada shell (`cmd`, `bash`, `powershell`) vive en un archivo de configuración externo, permitiendo al usuario añadir shells personalizadas. `axes` lo autogenera en la primera ejecución.
2. **El Script Temporal:**
    - `axes` crea un archivo de script temporal (`.bat` o `.sh`).
    - Escribe en este script los comandos necesarios en el formato correcto para la shell objetivo:
        - Comandos para silenciar la salida (`@echo off`).
        - Todos los `set`/`export` de la sección `[env]` del proyecto.
        - La ejecución (`call`/`source`) del hook `at_start`.
        - Un mensaje de bienvenida.
    - La shell se lanza con argumentos que le dicen que ejecute este script y luego permanezca interactiva (ej. `cmd.exe /K <script.bat>`).
3. **Ejecución de `at_exit`:** Cuando el proceso de la sub-shell termina (el usuario escribe `exit`), `axes` recupera el control y ejecuta el comando definido en `at_exit`, ideal para tareas de limpieza.
4. **Manejo de `Ctrl+C`:** El proceso principal de `axes` intercepta la señal `Ctrl+C`. En lugar de terminar abruptamente, establece un flag atómico. Si la sub-shell termina debido a la interrupción, `axes` lo detecta y sale de forma limpia con el código de estado `130`, evitando dejar la terminal del usuario en un estado inconsistente.

## La Máquina de Estados de Onboarding (`onboarding_manager`)

El comando `register` es la puerta de entrada a una máquina de estados compleja diseñada para integrar proyectos existentes de forma segura e inteligente.

**Flujo Principal:**

1. **Análisis:** Comprueba si el directorio es un proyecto `axes` y si ya está registrado.
2. **Detección de Identidad:** Intenta leer `project_ref.bin`.
3. **Dos Caminos Principales:**
    - **Si `.bin` existe:** Entra en un flujo de **validación y reconciliación**. Comprueba el UUID, el padre y el nombre contra el índice global. Si encuentra conflictos, en modo interactivo, le pregunta al usuario cómo resolverlos (ej. "¿Actualizar ruta?", "¿Elegir nuevo padre?").
    - **Si `.bin` no existe:** Entra en un flujo de **descubrimiento**. En modo interactivo, pregunta al usuario por el nombre y el padre del proyecto para poder crear una nueva identidad.
4. **Modo No Interactivo (`--autosolve`):** Sigue los mismos flujos, pero en lugar de preguntar, falla con un error si encuentra cualquier ambigüedad (conflicto de UUID, enlace roto, nombre duplicado, o falta de `project_ref.bin` en una llamada de nivel superior).
5. **Recursión:** Después de registrar con éxito un proyecto, escanea sus subdirectorios en busca de otros proyectos `axes` no registrados y se llama a sí mismo para cada uno, pasando el UUID del padre actual como sugerencia para el nuevo hijo. Esto permite registrar un monorepo entero de forma semi-automática.
