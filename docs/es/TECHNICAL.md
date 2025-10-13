<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../TECHNICAL.md">English</a> • 
  <a href="./TECHNICAL.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../TECNICAL.md), que es la fuente canónica de la documentación.

# 1. Introducción y Filosofía de Diseño

Este documento proporciona un análisis técnico en profundidad de la arquitectura interna de `axes`. A diferencia de la documentación para el usuario, su propósito es detallar las decisiones de diseño, los patrones de software y las estrategias de optimización que permiten a `axes` cumplir sus objetivos de robustez y rendimiento.

## 1.1. El Problema Técnico Abordado

Los *task runners* tradicionales operan en un modelo basado en texto y sin estado. Este enfoque, aunque simple, introduce cuellos de botella fundamentales a medida que la complejidad del proyecto escala:

1. **Sobrecarga del *Parsing* de la Ruta Caliente (Hot Path):** Cada ejecución requiere leer y analizar archivos de configuración de texto (ej. `Makefile`, `Justfile`, `package.json`), una operación intensiva en I/O y CPU que se repite innecesariamente.
2. **Gestión Implícita de Dependencias:** La relación entre diferentes componentes de un monorepo (ej. `api` depende de `common-lib`) no está formalizada, lo que conduce a flujos de trabajo frágiles y a la falta de herencia de configuración.
3. **Falta de Identidad Persistente:** Identificar un proyecto basándose en su ruta de sistema de archivos es inherentemente volátil. Operaciones como renombrar o mover un directorio rompen flujos de trabajo y referencias.

`axes` fue diseñado desde cero para resolver estos problemas a nivel arquitectónico.

### 1.2. Los Tres Pilares de la Arquitectura de `axes`

La arquitectura de `axes` se sustenta en tres principios fundamentales que trabajan sinérgicamente para ofrecer un rendimiento de élite y una robustez estructural.

#### 1.2.1. Estado Centralizado y Persistente (`GlobalIndex`)

El núcleo de `axes` es un **índice global** (`GlobalIndex`), una base de datos binaria compacta que actúa como la **Única Fuente de Verdad** para todo el ecosistema del proyecto. Este índice mapea un **UUID inmutable** para cada proyecto a sus metadatos esenciales, como su ruta física, nombre y relación padre-hijo.

- **Rendimiento de Inicio:** Al utilizar un formato binario (`bincode`), la deserialización del índice completo en memoria es órdenes de magnitud más rápida que analizar un equivalente en formato texto (JSON, TOML). Esto minimiza drásticamente la latencia de arranque en frío (*cold-start*).
- **Robustez Estructural:** Al desacoplar la identidad lógica (UUID) de la ubicación física (ruta), el sistema se vuelve resiliente a los cambios en el sistema de archivos.

#### 1.2.2. Carga Perezosa y Concurrente (Patrón `Facade`)

`axes` opera bajo el principio de "mínimo trabajo necesario". La lectura y compilación de los archivos `axes.toml` no ocurre por adelantado. En su lugar, se construye una estructura ligera en memoria, la `ResolvedConfig`, que actúa como una **Fachada (*Facade*)**.

- **Resolución Bajo Demanda:** Los datos de configuración (scripts, variables, etc.) solo se cargan del disco y se combinan cuando se invoca un método como `get_script()` o `get_env()` por primera vez.
- **Concurrencia Optimizada:** El `ConfigLoader` utiliza un *thread pool* (`rayon`) para cargar y compilar concurrentemente las diferentes capas de la jerarquía de un proyecto. La sincronización se gestiona eficientemente mediante *promises* (`Arc<OnceLock<...>>`), asegurando que cada capa se compile solo una vez, incluso bajo demanda concurrente.

#### 1.2.3. Compilación Anticipada (AOT) y Caché AST

Este es el pilar más crítico para el rendimiento en ejecuciones "calientes" (*hot path*). `axes` no es un intérprete; es un compilador de flujos de trabajo con una caché persistente.

- **Compilación a AST:** En la primera ejecución ("camino frío"), `axes` analiza los archivos `axes.toml` y compila los *scripts* y variables a una representación intermedia optimizada: un **Árbol de Sintaxis Abstracta (AST)**, materializado en nuestras *structs* `Task`.
- **Caché Binario Persistente:** Este AST se guarda en un caché binario (`.bin`).
- **Ejecuciones Instantáneas ("Camino Caliente"):** Las ejecuciones posteriores omiten por completo el costoso análisis de texto. `axes` deserializa el AST precompilado desde el caché binario—una operación órdenes de magnitud más rápida que el análisis de texto—y lo ejecuta instantáneamente.

**El resultado: pagas el coste de orquestación una vez. Obtienes la velocidad de un ejecutor simple cada vez después.**

- ⚙️ **[Inmersión Profunda en la Arquitectura (`TECHNICAL.md`)](./TECNICAL.md)**: Para aquellos interesados en la ingeniería detrás de nuestro rendimiento.

### 1.3. Diagrama de Flujo: Camino Frío vs. Camino Caliente

El siguiente diagrama ilustra la diferencia fundamental en el flujo de trabajo entre la primera ejecución de un *script* y las ejecuciones posteriores.

```mermaid
graph TD
    subgraph "Ciclo de Vida de la Configuración en `axes`"
        
        A["<br><b>Inicio</b><br>Se ejecuta el comando axes"] --> B{"<br>¿El hash de <code>axes.toml</code> coincide<br>con el hash en <code>GlobalIndex</code>?"}

        B -- "<b>❄️ No (Camino Frío / Fallo de Caché)</b>" --> C_IO["<br><b>[I/O de Disco + CPU]</b><br>1. Leer <code>axes.toml</code>"]
        C_IO --> C_CPU["<br><b>[Intensivo en CPU]</b><br>2. Parsear TOML y Compilar Scripts a AST (`Task`)"]
        C_CPU --> D_IO["<br><b>[I/O de Disco]</b><br>3. Serializar y Escribir AST a Caché Binaria (<code>.bin</code>)"]
        D_IO --> E["<br><b>[En Memoria]</b><br>Usar el AST recién compilado"]
        
        B -- "<b>🔥 Sí (Camino Caliente / Éxito de Caché)</b>" --> H_IO["<br><b>[Mínima I/O de Disco]</b><br>1. Leer Caché Binaria (<code>.bin</code>)"]
        H_IO --> H_CPU["<br><b>[Mínima CPU]</b><br>2. Deserializar AST desde binario"]
        H_CPU --> E
        
        E --> F["[Independiente de axes]<br><b>Ejecución</b><br>El `TaskExecutor` opera sobre el AST en memoria"]
        F --> G["<br><b>Fin</b><br>"]

    end

    %% Nodos de bajo coste (operaciones en memoria, decisiones)
    style A fill:#e6f7ff,stroke:#0050b3,stroke-width:1px,color:#055
    style B fill:#e6f7ff,stroke:#0050b3,stroke-width:2px,color:#055
    style E fill:#e6f7ff,stroke:#0050b3,stroke-width:1px,color:#055
    style F fill:#808080,stroke:#0050b3,stroke-width:2px
    style G fill:#f0f0f0,stroke:#595959,stroke-width:1px,color:#055

    %% Nodos de Camino Caliente (I/O y CPU optimizados)
    style H_IO fill:#d9f7be,stroke:#237804,stroke-width:2px,color:#055
    style H_CPU fill:#d9f7be,stroke:#237804,stroke-width:1px,color:#055
    
    %% Nodos de Camino Frío (Alto Coste)
    style C_IO fill:#fff1b8,stroke:#d48806,stroke-width:2px,color:#055
    style C_CPU fill:#ffd8bf,stroke:#d46b08,stroke-width:2px,color:#055
    style D_IO fill:#ffccc7,stroke:#cf1322,stroke-width:2px,color:#055
```

Esta arquitectura de compilación y *caching* es lo que nos permite ofrecer el poder de una orquestación compleja a una velocidad que rivaliza con la de los ejecutores más simples. Además, el uso de *hashes* para los nombres de los archivos de caché permite que este caché sea **compartido entre miembros del equipo** a través de una unidad de red o un sistema de *caching* distribuido, asegurando que el coste de compilación se pague **una sola vez para todo el equipo**.

## 2. Anatomía de la Ejecución de Comandos: El Ciclo de Vida de un Comando

El proceso de ejecución de comandos en `axes` está coreografiado rigurosamente para maximizar la velocidad, la seguridad y el consumo perezoso de recursos.

### 2.1. El *Dispatcher* Universal y la Gramática

El binario de `axes` recibe todos los argumentos en un vector (`Vec<String>`) y utiliza una gramática universal (implementada en `bin/axes.rs`) para determinar la intención del usuario. Esta lógica tiene tres reglas de decisión principales (Contexto, Acción, Argumentos) y es el punto donde se decide qué parte de la entrada se interpretará como el contexto (`<ctx>`) y qué parte como comandos para el *handler* (`[args...]`).

### 2.2. Resolución de Contexto y Persistencia de Identidad (`core/context_resolver.rs`)

Antes de cargar cualquier configuración, el sistema debe saber sobre qué proyecto está operando.

1. **Prioridad de Resolución:** El `context_resolver` transforma una entrada de texto (ej. `mi-app/api` o `g!`) en el **UUID** canónico del proyecto. La resolución sigue un orden estricto de prioridad: Alias (`g!`, `db!`), Navegación Relativa (`.`, `..`, `*`, `**`), y finalmente Nombres de Proyectos (búsqueda jerárquica).
2. **Referencia Local (`ProjectRef`):** El sistema mantiene un archivo binario de referencia local (`project_ref.bin`) en cada directorio de proyecto (`.axes/`). Este archivo almacena el `UUID` propio del proyecto, el `UUID` de su padre y su nombre simple. Si el índice global se corrompe, `axes` puede reconstruir la identidad del proyecto a partir de esta referencia local, asegurando la autorreparación del sistema.
3. **Optimización `last_used`:** Cada resolución de contexto exitosa actualiza los *punteros de caché* (`last_used`, `last_used_child`) en el `GlobalIndex` para acelerar futuras búsquedas (`**` y `*`).

### 2.3. Carga Concurrente de Capas (`core/config_loader.rs`)

Una vez que se conoce el `UUID` del proyecto objetivo, la fachada `ResolvedConfig` inicia la fase de carga.

El `ConfigLoader` determina la cadena de herencia completa (desde el `UUID` objetivo hasta la raíz `global`) y orquesta la carga de las capas de configuración desde estas cadenas concurrentemente para minimizar la latencia.

#### Diagrama: Flujo de Carga de Capas

```mermaid
graph TD
    A["ResolvedConfig.get_env()"] --> B("ConfigLoader");
    B --> C("Identificar Jerarquía: [UUID_A, UUID_P, UUID_G]");

    C --> D_R(Rayon::scope);
    
    D_R --> E1("Tarea 1: load_layer_task(UUID_A)");
    D_R --> E2("Tarea 2: load_layer_task(UUID_P)");
    D_R --> E3("Tarea 3: load_layer_task(UUID_G)");

    E1 --> F1{"¿Hit/Miss de Caché?"};
    E2 --> F2{"¿Hit/Miss de Caché?"};
    E3 --> F3{"¿Hit/Miss de Caché?"};

    F1 --> G_A("LayerPromise.set(Result<Arc<Task>>)");
    F2 --> G_P("LayerPromise.set(Result<Arc<Task>>)");
    F3 --> G_G("LayerPromise.set(Result<Arc<Task>>)");

    G_A --> H("ResolvedConfig.get_layer(UUID_A)");
    G_P --> H;
    G_G --> H;

    H --> I["Fusión de Datos y Memoización"];
    I --> J["Resultado Final"];

    style D_R fill:#d9f7be,stroke:#237804,stroke-width:2px,color:#055
    style E1 fill:#fff1b8,stroke:#d48806,stroke-width:1px,color:#055
    style E2 fill:#fff1b8,stroke:#d48806,stroke-width:1px,color:#055
    style E3 fill:#fff1b8,stroke:#d48806,stroke-width:1px,color:#055

    %% Sincronización eficiente usando Arc/OnceLock
    H -.-> G_A; 
    H -.-> G_P; 
    H -.-> G_G;
```

#### Mecanismos de Sincronización

1. **`LayerPromise` (`Arc<OnceLock<...>>`):** Cada tarea de carga de capa es asíncrona. La `ResolvedConfig` obtiene una "promesa" del resultado. El uso de `OnceLock` es crucial: si un *thread* ya está calculando la caché para una capa, cualquier otro *thread* que la necesite simplemente **se bloquea y espera** en el mismo `OnceLock`. Esto asegura que la costosa operación de `Cache Miss` (I/O + Compilación) nunca se duplique, incluso en entornos altamente concurrentes.
2. **Manejo de `Cache Miss`:** Si se detecta un `Cache Miss` (el `axes.toml` ha cambiado), la tarea de carga procede a la compilación y produce una `IndexUpdate`. Estos *updates* son recolectados por el *thread* principal y aplicados al `GlobalIndex` de forma secuencial (antes de que la aplicación termine), garantizando la seguridad de la caché.

### 2.4. El Modelo de Comando: Compilación a AST

La compilación es el paso donde el texto del usuario se transforma en una estructura de datos optimizada y ejecutable.

1. **De TOML a AST:** `axes` convierte la flexible `ProjectConfig` (el formato de texto) en una `CachedProjectConfig`. Este proceso implica transformar cada `Command` en una `Task`, que es nuestra representación materializada y optimizada del AST. Una `Task` contiene una secuencia de `CommandExecution`.
2. **Propósito de `Task`:** Almacenar *scripts* pre-analizados y *tokens* resueltos (`TemplateComponent`), junto con metadatos de ejecución (`ignore_errors`, `run_in_parallel`). Esto elimina la necesidad de usar `shlex` y el análisis de plantillas en tiempo de ejecución.
3. **Separación de Modelos:** La caché binaria (`bincode`) solo almacena la `Task` compilada (y no el tipo `Command` intermedio), asegurando que la serialización binaria sea segura, ultrarrápida y unívoca.

## 3. Estructuras de Datos Fundamentales y su Diseño

La robustez y el rendimiento de `axes` no son solo el resultado de algoritmos, sino también del diseño deliberado de sus estructuras de datos. Cada `struct` ha sido diseñada para un propósito específico dentro del ciclo de vida de la aplicación.

### 3.1. Dualidad de Estado: `GlobalIndex` vs. `ProjectRef`

`axes` gestiona el estado en dos niveles: uno global y uno local, creando un sistema resiliente y autorreparable.

- **`GlobalIndex` (El Mapa Global):**
  - **Estructura:** Un único archivo binario (`index.bin`) que contiene principalmente un `HashMap<Uuid, IndexEntry>`.
  - **Propósito:** Actúa como el índice principal para todas las operaciones de búsqueda y resolución de contexto. Permite la resolución UUID a metadatos (ruta, nombre, relación padre-hijo) en tiempo constante O(1).
  - **Optimización de Alias:** Mantiene un `HashMap<String, Uuid>` separado para los alias. Esta es una decisión de diseño crítica: desacopla los "atajos" de la estructura jerárquica principal. Permite relaciones de alias de muchos a uno (múltiples alias pueden apuntar al mismo proyecto), una flexibilidad que se perdería si el alias fuera una propiedad de la `IndexEntry`.

- **`ProjectRef` (La Identidad Local):**
  - **Estructura:** Un pequeño archivo binario (`project_ref.bin`) dentro de cada directorio de proyecto (`.axes/`).
  - **Propósito:** Actúa como una "etiqueta de identidad" inmutable para el proyecto. Almacena su propio `self_uuid`, `name` y `parent_uuid`.
  - **Robustez y Autorreparación:** Este archivo es la clave de la resiliencia de `axes`. Si el `GlobalIndex` se corrompe o se elimina, el comando `axes register` puede recorrer el sistema de archivos y utilizar los archivos `project_ref.bin` para **reconstruir el índice global con fidelidad completa**. Permite que un proyecto se mueva o se renombre en el sistema de archivos y luego se "registre de nuevo" sin perder su identidad histórica ni sus relaciones.

### 3.2. La Cadena de Transformación de Comandos: De Texto a AST

Lograr tanto la flexibilidad para el usuario como el rendimiento para el ejecutor reside en la clara separación y el diseño intencional de sus estructuras de datos.

```mermaid
graph LR
    A("<b>1. Usuario</b><br><code>axes.toml</code>") --> B{"<b>2. Deserializador TOML</b><br>(<code>serde_toml</code>)"};
    
    subgraph "Fase de Carga y Compilación (Fallo de Caché)"
        B --> C["<b>3. Modelo Flexible: <code>ProjectConfig</code></b><br>Usa <code>TomlCommand</code> y <code>TomlOpenWithConfig</code> con <code>#[serde(flatten)]</code> para máxima flexibilidad sintáctica."];
        C --> D["<b>4. Modelo Canónico: <code>CanonicalCommand</code></b><br>Normaliza todas las variantes de sintaxis (simple, secuencia, por plataforma) en una única estructura estandarizada."];
        D --> E["<b>5. Modelo de Caché (AST): <code>CachedProjectConfig</code></b><br>Contiene `Tasks`. Los comandos han sido compilados a esta representación binaria optimizada. Es 100% compatible con <code>bincode</code>."];
    end
    
    E --> F{"<b>6. Serializador Binario</b><br>(<code>bincode</code>)"};
    F --> G("<b>7. Caché en Disco</b><br><code>.bin</code>");

    subgraph "Fase de Ejecución (Éxito de Caché)"
        G --> H{"<b>8. Deserializador Binario</b><br>(<code>bincode</code>)"};
        H --> I["<b>9. Modelo de Caché en Memoria: <code>CachedProjectConfig</code></b><br>El AST se carga directamente, sin análisis de texto."];
    end
    
    I --> J("<b>10. <code>TaskExecutor</code></b><br>Opera directamente sobre el AST en memoria.");

    style A fill:#f0f0f0,stroke:#333,color:#055
    style G fill:#f0f0f0,stroke:#333,color:#055
    style C fill:#e6f7ff,stroke:#096dd9,color:#055
    style D fill:#bae7ff,stroke:#096dd9,color:#055
    style E fill:#d9f7be,stroke:#237804,color:#055
    style I fill:#d9f7be,stroke:#237804,color:#055
```

- **`TomlCommand` y `TomlOpenWithConfig`:** Son *structs* diseñadas para "solo lectura" con máxima flexibilidad de usuario, usando atributos como `#[serde(untagged)]` y `#[serde(flatten)]`. Su único propósito es deserializar `axes.toml` sin errores, aceptando múltiples formas sintácticas.
- **`Command` y `CanonicalCommand`:** Actúan como una capa de normalización. Después del análisis inicial, todas las variantes de `TomlCommand` se convierten en una `CanonicalCommand`. Esto simplifica la lógica de compilación posterior, ya que solo tiene que lidiar con una única estructura bien definida.
- **`Task`, `CommandExecution`, `TemplateComponent` (El AST):** Es el producto final de la compilación. Es una representación en memoria optimizada para la ejecución, que descompone cada comando en sus partes lógicas (literales, parámetros, subcomandos dinámicos). Esta es la estructura que se serializa con `bincode` en la caché. Al ser una `struct` regular sin atributos "mágicos" de `serde`, su serialización y deserialización binaria es determinista, ultrarrápida y robusta.

### 3.3. El Resolutor de Argumentos (`ArgResolver`)

El `ArgResolver` es el componente que une los parámetros definidos en un `Task` con los argumentos proporcionados por el usuario en la línea de comandos.

- **Pre-Parseo y Validación:** Antes de la ejecución, el sistema (`run::handle`, `start::handle`, etc.) recorre el `Task` aplanado y recopila **todas** las definiciones de parámetros (`ParameterDef`) en una única lista. Esta lista representa el "contrato" completo del *script*.
- **Resolución de Pasada Única:** El `ArgResolver` se construye una vez con este contrato y los argumentos del usuario. En su constructor, ejecuta toda la lógica de validación:
  - Comprueba que todos los parámetros `required` estén presentes.
  - Detecta conflictos, como el uso simultáneo de un *flag* y su alias (`--verbose` y `-v`).
  - Detecta argumentos inesperados si el *script* no usa el *token* genérico `<params>`.
- **Resultado Inmutable:** El `ArgResolver` produce un `HashMap` inmutable que mapea el *token* original (ej. `<params::0(required)>`) a su valor final resuelto. Durante la ejecución, el `TaskExecutor` simplemente realiza búsquedas rápidas en este mapa, sin necesidad de más análisis o validación.

### 3.4. El Sistema de Caché

- **Caché por Capas:** `axes` no tiene un único caché monolítico, sino una caché para cada `axes.toml` en la jerarquía del proyecto. Esto mejora la granularidad y reduce la invalidación: un cambio en `my-app/api/axes.toml` solo invalida la caché de `api`, no la de `my-app` o `global`.
- **Gestión de Caché:** El comando `axes <ctx> _cache clear` invalida la caché de una capa específica borrando su `config_hash` y `cache_dir` del `GlobalIndex`. La próxima vez que se necesite esa capa, se forzará una recompilación. Un futuro comando `axes cache gc` se encargará de limpiar de disco los archivos de caché binarios que ya no estén referenciados por ningún proyecto en el `GlobalIndex`.

## 4. Optimizaciones Adicionales y Conclusiones de Rendimiento

Más allá de los tres pilares arquitectónicos, `axes` implementa una serie de optimizaciones micro-arquitectónicas para minimizar la latencia en cada operación.

### 4.1. Patrón de Memoización en `ResolvedConfig`

La fachada `ResolvedConfig` no solo es perezosa a nivel de I/O de disco, sino también a nivel de cómputo. Las operaciones como fusionar variables de entorno a través de una jerarquía completa (`get_env()`) son costosas. Para evitar repetir este trabajo, `ResolvedConfig` utiliza un patrón de **memoización** interno.

- **Mecanismo:** Cada método costoso (ej. `get_env`, `get_options`) utiliza un campo `memoized_*` protegido por un `Mutex`.
  - En la **primera llamada**, el `Mutex` se bloquea, se realiza el costoso cálculo (fusión de `HashMap`s de todas las capas) y el resultado se almacena en el campo `memoized_*`.
  - En **llamadas posteriores**, el `Mutex` solo se bloquea brevemente para comprobar si el resultado ya existe, y lo devuelve instantáneamente.
- **Optimización con `Arc`:** Para resultados que son colecciones grandes (como el `HashMap` de `get_env`), el valor en caché se envuelve en un `Arc` (`Arc<HashMap<...>>`). El método devuelve un clon del `Arc`, que es un incremento atómico del contador de referencias (extremadamente rápido), en lugar de un clon completo del `HashMap` (extremadamente lento). Esta fue una optimización clave identificada a través de `flamegraph` para eliminar un cuello de botella severo.

### 4.2. Minimización de Llamadas al Sistema de Archivos

Las operaciones de I/O de disco y las llamadas al sistema son los mayores enemigos de la latencia en una herramienta de línea de comandos. `axes` minimiza activamente las llamadas al sistema:

- **Resolución de Contexto en Sesión:** Cuando un usuario está dentro de una sesión (`AXES_PROJECT_UUID` está definido), la resolución de contexto para referencias como `.` se realiza **enteramente en memoria**. En lugar de llamar a `dunce::canonicalize` para preguntar al sistema de archivos por el directorio actual, `axes` simplemente utiliza la ruta del proyecto de la sesión, que ya está cargada en el `GlobalIndex`.
- **Validación de Caché por Hash:** El sistema de caché no depende de *timestamps* de archivos, que pueden ser inconsistentes. Utiliza un hash criptográfico (`blake3`) del contenido del `axes.toml`. Esto no solo es más robusto, sino que en muchos sistemas operativos modernos leer un archivo pequeño para hashearlo puede ser más rápido que múltiples accesos a metadatos si el contenido ya está en la caché de páginas del sistema operativo.

### 4.3. Elección de Dependencias de Alto Rendimiento

La pila de dependencias de `axes` ha sido seleccionada con el rendimiento como criterio principal:

- **`bincode` vs. `serde_json`/`serde_toml`:** Para la serialización de caché e índice, `bincode` ofrece un rendimiento de deserialización muy superior en comparación con los formatos basados en texto, ya que no requiere un analizador léxico/sintáctico.
- **`rayon`:** Para la carga concurrente de capas, `rayon` proporciona un *thread pool* de "robo de trabajo" de clase mundial con una sobrecarga mínima, permitiendo una paralelización casi ideal de las tareas de I/O y compilación.
- **`clap`:** Se utiliza para el análisis de argumentos de la CLI. Su macro `derive` genera código de análisis altamente optimizado en tiempo de compilación, lo que resulta en un análisis de argumentos muy rápido en tiempo de ejecución.

### 4.4. Conclusión: Una Arquitectura Orientada al Rendimiento

Cada decisión de diseño en `axes` se ha tomado bajo la lente de la optimización del rendimiento, priorizando la velocidad en el "camino caliente" (la ejecución de comandos por parte del usuario).

- Hemos **desplazado los costes computacionales** del tiempo de ejecución al tiempo de compilación del caché (`Compilación AOT a AST`).
- Hemos **eliminado la redundancia** a través de la memoización (`ResolvedConfig`).
- Hemos **minimizado las operaciones lentas** como I/O y análisis de texto, reemplazándolas con lectura binaria y operaciones en memoria.

El resultado es un sistema que no solo *se siente* rápido, sino que está probado empíricamente que supera a sus competidores, proporcionando una base sólida y de alto rendimiento sobre la cual construir el futuro de la orquestación de flujos de trabajo.
