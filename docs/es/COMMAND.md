<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../COMMAND.md">English</a> •
  <a href="./COMMAND.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../COMMAND.md), que es la fuente canónica de la documentación.

# Referencia Completa de Comandos

Este documento es la guía de referencia definitiva para cada comando disponible en la CLI de `axes`. Para un tutorial guiado, consulta la [**Guía de Inicio Rápido (`GETTING_STARTED.md`)**](./GETTING_STARTED.md).

## Gramática Universal y Atajos

`axes` utiliza una gramática universal simple y predecible para interpretar tus comandos. Sigue un conjunto claro de reglas para determinar la acción que deseas realizar.

### **Las Reglas de la Gramática (en orden de prioridad):**

1. **`axes <contexto> <acción> [args...]`**
    * Si el **segundo** argumento es una acción conocida del sistema (`info`, `delete`, `start`, etc.), el primer argumento se trata como el contexto del proyecto.
    * *Ejemplo:* `axes mi-app info`

2. **`axes <acción> [args...]`**
    * Si el **primer** argumento es una acción conocida del sistema, `axes` la ejecuta. Esto se usa para comandos globales (`init`, `tree`, `alias`) o al usar un contexto implícito.
    * *Ejemplo (Global):* `axes tree --all`
    * *Ejemplo (Contexto Implícito):* `axes info` (equivalente a `axes . info`)

3. **`axes <ruta_script_virtual> [params...]` (Por Defecto)**
    * Si ninguna de las reglas anteriores coincide, `axes` asume que estás intentando **ejecutar un script**. Este es un atajo para `axes <ctx> run <nombre_script>`.
    * *Ejemplo:* `axes build --release` => `ejecutar script build del proyecto actual` (`axes run build --release`)
    * *Ejemplo:* `axes api/build --release` => `ejecutar script build del hijo 'api' desde el proyecto actual` (`axes api run build --release`)

### **Sistema de Contexto:**

* **Contexto Explícito:** `axes mi-app/api ...`
* **Contexto Implícito (`.`):** Para comandos que requieren un contexto (`info`, `run`, `start`, etc.), si no proporcionas uno, `axes` usa automáticamente `.` (el proyecto en el directorio actual o su primer ancestro).
* **Navegación:** Puedes usar `..` para referirte a un proyecto padre, `*` para el último hijo usado, y alias como `g!` para el proyecto global.

---

## Gestión del Ciclo de Vida del Proyecto

Estos comandos se utilizan para crear, registrar y eliminar proyectos del índice de `axes`. Son **destructivos** o **globales** y siguen la sintaxis `axes <acción> [args...]` o `axes <contexto> <acción> [args...]`.

### `init`

(Alias: `new`)

Inicializa `axes` en el directorio actual, creando una estructura `.axes/` y registrando el proyecto.

#### **Sintaxis**

```sh
axes init [opciones...]
```

#### **Argumentos y Banderas**

| Bandera                   | Descripción                                                                              | Requerido |
| :------------------------ | :--------------------------------------------------------------------------------------- | :-------- |
| `--parent <contexto>`     | El contexto del proyecto que será el padre. Acepta cualquier contexto válido (`..`, `mi-app/api`, `g!`, etc.). Por defecto es `global`. | No        |
| `--name <nombre>`         | El nombre para el nuevo proyecto. Si no se proporciona, se utiliza el nombre del directorio. | No        |
| `--version <ver>`         | La versión inicial para el proyecto (ej., `1.0.0`).                                     | No        |
| `--description <desc>`    | Una descripción breve para el proyecto.                                                  | No        |
| *...y otros*              | `init` acepta más banderas para pre-configurar `[vars]` y `[env]`.                         | No        |

#### **Ejemplos de Uso**

```sh
# Inicializa un proyecto en el directorio actual, y comienza el asistente preguntando por parámetros no indicados.
cd mi-proyecto
axes init

# Inicializa un proyecto especificando su padre por contexto, y el resto de parámetros se resolverán automáticamente.
cd mi-servicio
axes init --parent mi-monorepo --autosolve

# Inicializa un proyecto con todos los detalles desde la línea de comandos
axes init --name mi-api --parent .. --version "1.0-beta" --description "La API principal."
```

---

### `register`

(Alias: `reg`)

Registra un directorio que **ya contiene** una configuración `.axes/` en el índice global de `axes`. Es útil para incorporar proyectos existentes o reparar un registro roto.

#### **Sintaxis**

```sh
axes register [<ruta>] [--autosolve]
```

#### **Argumentos y Banderas**

| Argumento/Bandera   | Descripción                                                                                       | Requerido |
| :------------------ | :------------------------------------------------------------------------------------------------ | :------- |
| `<ruta>`            | La ruta al proyecto a registrar. Por defecto es el directorio actual.                               | No       |
| `--parent <contexto>`| Sugiere un padre para el proyecto registrado, anulando cualquier padre definido en su `project_ref.bin`. | No       |
| `--autosolve`       | Modo no interactivo. Falla ante cualquier conflicto.                                                | No       |

#### **Ejemplos de Uso**

```sh
# Registra el proyecto en el directorio actual interactivamente
axes register

# Registra un proyecto ubicado en otra ruta
axes register ../otro-proyecto-con-axes
```

---

### `unregister`

(Alias: `unreg`)

Elimina uno o más proyectos del índice de `axes`. **Esta acción NO elimina ningún archivo**, solo hace que `axes` "olvide" los proyectos.

#### **Sintaxis**

```sh
axes <contexto> unregister [--recursive] [--reparent-to <nuevo_padre>]
```

#### **Comportamiento por Defecto**

Por defecto, `unregister` **no es recursivo**. Solo desregistra el proyecto especificado en `<contexto>`, y sus hijos directos son reasignados al proyecto raíz (generalmente `global`) para evitar romper el grafo.

#### **Argumentos y Banderas**

| Bandera                     | Descripción                                                                                               | Requerido |
| :-------------------------- | :-------------------------------------------------------------------------------------------------------- | :-------- |
| `--recursive`               | Modo recursivo. Desregistra el proyecto especificado Y **todos sus descendientes**. No ocurre reasignación. | No        |
| `--reparent-to <padre>` | En lugar de mover los hijos a la raíz, los mueve al `<nuevo_padre>` especificado. No compatible con `--recursive`. | No        |

#### **Ejemplos de Uso**

```sh
# Desregistra `servicio-legado`, sus hijos ahora serán hijos de `global`.
axes mi-app/servicio-legado unregister

# Desregistra `prototipo` y todos sus subproyectos.
axes prototipo unregister --recursive

# Desregistra el "contenedor" `frontend-v1`, moviendo sus hijos a `frontend-v2`.
axes frontend-v1 unregister --reparent-to frontend-v2
```

---

### `delete`

(Alias: `del`)

☢️ **ACCIÓN DESTRUCTIVA.** Elimina el directorio `.axes/` del proyecto (y opcionalmente el de sus hijos) Y lo desregistra del índice.

#### **Sintaxis**

```sh
axes <contexto> delete [--recursive]
```

#### **Comportamiento por Defecto**

Al igual que `unregister`, `delete` **no es recursivo por defecto** para prevenir la pérdida accidental de datos. Solo elimina el `.axes/` del proyecto especificado, y sus hijos son reasignados al proyecto raíz.

#### **Argumentos y Banderas**

| Bandera       | Descripción                                                                                  | Requerido |
| :------------ | :------------------------------------------------------------------------------------------- | :-------- |
| `--recursive` | Modo recursivo. Elimina el `.axes/` del proyecto especificado Y **todos sus descendientes**.   | No        |

#### **Ejemplos de Uso**

```sh
# Elimina la identidad de `servicio-antiguo`, preservando a sus hijos.
axes servicio-antiguo delete

# Elimina completamente el proyecto `experimento` y todo lo que contiene del ecosistema `axes`.
axes experimento delete --recursive
```

## Inspección y Navegación

Estos comandos te ayudan a visualizar y comprender la estructura de tu árbol de proyectos y la configuración de cada uno. Son operaciones de solo lectura y completamente seguras.

### `tree`

(Alias: `ls`)

Muestra una representación visual del árbol de proyectos registrados, comenzando desde la raíz o un proyecto específico.

#### **Sintaxis**

```sh
axes [<contexto>] tree [-p, --paths] [-u, --uuids] [--all]
```

#### **Comportamiento**

* Si se proporciona `<contexto>`, muestra el subárbol desde ese proyecto.
* Si se omite, muestra el árbol completo desde el proyecto en el directorio actual (`.`). Para ver el árbol completo, usa `axes global tree` o `axes g! tree`.

#### **Argumentos y Banderas**

| Argumento/Bandera | Descripción                                                                 | Requerido |
| :------------------ | :------------------------------------------------------------------------- | :-------- |
| `<contexto>`        | El proyecto desde el cual comenzar a mostrar el árbol.                          | No        |
| `-p`, `--paths`     | Muestra la ruta física absoluta de cada proyecto.                          | No        |
| `-u`, `--uuids`     | Muestra el UUID único de cada proyecto.                                    | No        |
| `--all`             | Un atajo para mostrar toda la información disponible (`--paths` y `--uuids`).| No        |
| `-d`, `--depth <DEPTH>` | Limita la profundidad de la visualización del árbol.                       | No        |
| `--check`           | Comprueba si las rutas de los proyectos existen en el sistema de archivos. | No        |

#### **Ejemplos de Uso**

```sh
# Muestra el árbol de proyectos completo
axes tree

# Muestra el subárbol del monorepo `mi-app`
axes mi-app tree

# Muestra el árbol completo con rutas y UUIDs, útil para depuración
axes tree --all

# Muestra el árbol del padre del proyecto actual
axes .. tree -p
```

---

### `info`

Muestra un resumen completo de la configuración **final y fusionada** de un proyecto, incluyendo metadatos, scripts heredados y variables.

#### **Sintaxis**

```sh
axes [<contexto>] info
```

#### **Argumentos y Banderas**

| Argumento   | Descripción                                   | Requerido |
| :---------- | :-------------------------------------------- | :-------- |
| `<contexto>`| El proyecto cuya información se va a mostrar. | No        |

#### **Ejemplos de Uso**

```sh
# Muestra la información del proyecto raíz
axes global info

# Muestra la configuración completa del servicio API, incluyendo
# las variables y scripts que ha heredado de `mi-app`.
axes mi-app/api info
```

La salida de `info` es tu mejor herramienta para depurar por qué un script se comporta de cierta manera o de dónde proviene una variable específica.

---

### `alias`

Gestiona atajos (alias) para las rutas de contexto de tus proyectos. Los alias son globales y te permiten acceder rápidamente a proyectos anidados profundamente.

#### **Sintaxis**

```sh
axes [<ctx>] alias <subcomando> [argumentos...]
```

#### **Subcomandos de `alias`**

| Argumento   | Descripción                                        | Requerido                  |
| :---------- | :------------------------------------------------- | :------------------------- |
| `set`       | Establece un nuevo alias o actualiza uno existente. | `<alias> <contexto_destino>` |
| `list`      | Enumera todos los alias definidos.                 | Ninguno                    |
| `remove`    | Elimina un alias.                                  | `<alias>`                  |
| `check`     | Verifica todos los alias, reportando enlaces rotos | Ninguno                    |

**`list`**
(Alias: `ls`)
Muestra una tabla de todos los alias definidos. Este es el subcomando predeterminado si no se especifica ninguno.

* **Sintaxis:** `axes alias [list]`

**`set`**
Crea un nuevo alias o actualiza uno existente.

* **Sintaxis:** `axes alias set <nombre_alias> <contexto_destino>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo (ej., `api`, `frontend`). No incluyas el `!`.
  * `<contexto_destino>`: La ruta completa del proyecto al que apuntará el alias.

**`remove`**
(Alias: `rm`)
Elimina un alias.

* **Sintaxis:** `axes alias rm <nombre_alias>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo a eliminar.

#### **Notas Importantes**

* Los alias se utilizan añadiendo un `!` al final. Por ejemplo, si creas `axes alias set api mi-monorepo/services/main-api`, puedes usarlo con `axes api! info`.
* El alias `g!` es un alias especial por defecto que siempre apunta al proyecto raíz. Puede ser modificado o eliminado, pero se mostrará una advertencia.

#### **Ejemplos de Uso**

```sh
# Lista todos los alias
axes alias

# Crea un atajo para un servicio anidado
axes alias set api mi-monorepo/services/api-v2

# Usa el nuevo alias
axes api! test

# Elimina un alias
axes alias rm api
```

## Interacción y Ejecución de Proyectos

Estos son los comandos principales que utilizarás en tu flujo de trabajo diario para ejecutar tareas, iniciar entornos y abrir tus proyectos.

### `run`

Ejecuta un script definido en el `axes.toml` de un proyecto. Este es el comando más potente y utilizado de `axes`.

#### **Sintaxis (Gramática Universal)**

La forma recomendada de ejecutar un script es utilizando la gramática universal, que se siente como un comando nativo:

```sh
axes [<contexto>]/<nombre_script> [parámetros...]
```

Este es un atajo ergonómico para la forma más explícita:

```sh
axes [<contexto>] run <nombre_script> [parámetros...]
```

#### **Argumentos y Banderas**

| Argumento         | Descripción                                                                  |
| :---------------- | :--------------------------------------------------------------------------- |
| `<contexto>`      | El contexto del proyecto. Por defecto es `.` (proyecto actual) si se omite.  |
| `<nombre_script>` | El nombre del script a ejecutar.                                             |
| `[parámetros...]` | Cualquier argumento adicional pasado directamente al motor de parámetros del script. |
| `--dry-run`       | Una bandera especial que imprime los comandos que *se ejecutarían* para la plataforma actual, sin ejecutarlos. Debe pasarse antes de los parámetros. |

#### **Funcionalidad Clave**

El comando `run` está orquestado por un potente motor de expansión de texto. Dentro de tus scripts, puedes usar una sintaxis especial `<...>` para:

* **Incluir variables:** `<vars::mi_variable>`
* **Componer otros scripts:** `<scripts::otro_script>`
* **Ejecutar comandos y sustituir su salida:** `<run('git rev-parse --short HEAD')>`
* **Pasar parámetros de CLI de forma estructurada:** `<params::0>`, `<params::fl(map='--flag', default='some', required)>`, `<params>`

> **Nota:** El sistema de scripting y parámetros es la característica más profunda de `axes`. Para una guía completa con ejemplos, consulta **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

#### **Ejemplos de Uso**

```sh
# Ejecuta el script 'build' en el proyecto actual (usando atajo)
axes build

# Ejecuta el script 'build' en el proyecto `mi-app/frontend`
axes mi-app/frontend run build

# Ejecuta el script 'test' y pasa un parámetro
# (asumiendo que `test` usa `<params>` o `<params::0>`)
axes mi-app/api/test tests/unit/test_auth.py

# Ejecuta un script pasando una bandera
# (asumiendo que `deploy` usa `<params::production>`)
axes mi-app/deploy --production
```

---

### `start`

Inicia una sesión de shell interactiva y persistente dentro del contexto de un proyecto. Es la herramienta ideal para el trabajo enfocado.

#### **Sintaxis**

```sh
axes [<contexto>] start [parámetros...]
```

#### **Argumentos y Banderas**

| Argumento         | Descripción                                                                         | Requerido |
| :---------------- | :---------------------------------------------------------------------------------- | :------- |
| `<contexto>`      | El proyecto en el que iniciar la sesión. (Implícitamente `.` si se omite).          | No       |
| `[parámetros...]` | Cualquier argumento adicional que se pasará a los hooks `at_start` y `at_exit`.     | No       |
| `--dry-run`       | Imprime los comandos de hook que *se ejecutarían*, sin iniciar una sesión.          | No       |

#### **Comportamiento de la Sesión**

Al ejecutar `start`, `axes` hace lo siguiente:

1. **Resuelve y Valida Parámetros:** `axes` analiza los `[parámetros...]` proporcionados y los valida contra las definiciones `<params::...>` encontradas en los hooks `at_start` y `at_exit`.
2. **Ejecuta el Hook `at_start`:** El script `at_start` se ejecuta, inyectando los parámetros resueltos.
3. **Inicia el Shell:** El shell interactivo se lanza con todas las variables `[env]` inyectadas.

Una vez dentro, puedes ejecutar comandos `axes` sin especificar el contexto. Al salir de la sesión con `exit`, se ejecuta el hook `at_exit`, que también recibe los mismos `[parámetros...]` resueltos al inicio de la sesión.

#### **Ejemplos de Uso**

```sh
# Inicia una sesión simple en el servicio API
axes mi-app/api start

# Asumiendo un `at_start` como: "docker-compose up -d <params::service>"
# Inicia una sesión y especifica qué servicio activar
axes mi-app/api start --service web
```

---

### `open`

Abre el directorio raíz de un proyecto usando una aplicación preconfigurada.

#### **Sintaxis**

```sh
axes [<contexto>] open [<clave_app>] [parámetros...]
```

#### **Argumentos y Banderas**

| Argumento         | Descripción                                                                              | Requerido |
| :---------------- | :--------------------------------------------------------------------------------------- | :------- |
| `<contexto>`      | El proyecto a abrir. (Implícitamente `.` si se omite).                                   | No       |
| `[<clave_app>]`   | La clave de la aplicación a usar (ej., `code`). Si se omite, se usa la clave `default`. | No       |
| `[parámetros...]` | Cualquier argumento adicional que se pasará al script de la `clave_app`.                 | No       |

#### **Configuración**

Las aplicaciones se definen en la sección `[options.open_with]` de tu `axes.toml`. Cada entrada es un **script completo de `axes`**, lo que permite lógica específica de la plataforma, descripciones y parametrización.

```toml
[options.open_with]
# Establece la acción por defecto cuando se ejecuta `axes open` sin clave_app.
default = "vsc"

[options.open_with.vsc]
desc = "Abre el proyecto en Visual Studio Code."
run = 'code "<path>"'

[options.open_with.terminal]
desc = "Abre un nuevo terminal en la raíz del proyecto."
run = 'wt -d "<path>/<params::0(default=".")>"' # Ejemplo para Windows Terminal

[options.open_with.explorer]
desc = "Abre el proyecto en el explorador de archivos del sistema."
windows = "explorer \"<path>\""
macos = "open \"<path>\""
default = "xdg-open \"<path>\"" # Para Linux
```

#### **Ejemplos de Uso**

```sh
# Abre el proyecto actual con la aplicación por defecto (implícitamente axes . open)
axes open

# Abre explícitamente el proyecto `mi-app/api` en el explorador de archivos
# (Asumiendo que la clave 'files' está definida)
axes mi-app/api open files

# Usa el atajo 'terminal' y pasa un parámetro para abrir en el subdirectorio 'src'
axes mi-app/frontend open terminal src
```

## Refactorización del Árbol de Proyectos

Estos comandos te permiten modificar la estructura de tu ecosistema `axes`, cambiando las relaciones entre proyectos y sus nombres. Estas son operaciones potentes que actualizan el índice global de `axes`.

### `link`

Cambia el padre de un proyecto existente, moviéndolo a una nueva ubicación en el árbol lógico. Esta operación es puramente estructural y no mueve ningún archivo en tu disco.

#### **Sintaxis**

```sh
axes <contexto_hijo> link <contexto_nuevo_padre>
```

#### **Argumentos y Banderas**

| Argumento                  | Descripción                                       | Requerido |
| :------------------------- | :------------------------------------------------ | :-------- |
| `<contexto_hijo>`          | El proyecto que deseas mover.                     | Sí        |
| `<contexto_nuevo_padre>`   | El proyecto que se convertirá en su nuevo padre. | Sí        |

#### **Validaciones de Seguridad**

`link` es una operación segura. `axes` prevendrá cualquier acción que pueda corromper el grafo del proyecto, fallando con un error claro si intentas:

* **Crear un ciclo:** Mover un proyecto para que se convierta en su propio descendiente (ej., `axes A link A/B`).
* **Crear una colisión de nombres:** Mover un proyecto a un nuevo padre que ya tiene un hijo con el mismo nombre.

#### **Ejemplos de Uso**

```sh
# `servicio-legado` era hijo de `global`, ahora será hijo de `monorepo-v2`.
axes servicio-legado link monorepo-v2

# Mueve el `panel-admin` para que sea hijo del servicio `api` en lugar de `frontend`.
axes mi-app/frontend/panel-admin link mi-app/api
```

---

### `rename`

Cambia el nombre de un proyecto. Este es el nombre utilizado en las rutas de contexto, no el nombre del directorio en el disco.

#### **Sintaxis**

```sh
axes <contexto> rename <nuevo_nombre>
```

#### **Argumentos y Banderas**

| Argumento         | Descripción                                   | Requerido |
| :---------------- | :-------------------------------------------- | :-------- |
| `<contexto>`      | El proyecto que deseas renombrar.             | Sí        |
| `<nuevo_nombre>`  | El nuevo nombre para el proyecto.             | Sí        |

#### **Reglas de Nomenclatura**

El `<nuevo_nombre>` debe seguir ciertas reglas para garantizar la estabilidad:

* **No puede contener espacios** o caracteres de ruta (`/`, `\`).
* **No puede ser un nombre reservado** para navegación (ej., `.` , `..`, `*`).

`axes` también te advertirá si intentas usar nombres que, aunque válidos, no son recomendados (ej., que empiecen por `-`). Renombrar el proyecto raíz `global` está permitido, pero requerirá una confirmación adicional debido a su importancia.

#### **Ejemplos de Uso**

```sh
# Renombra un proyecto de `api-v1` a `api-legacy`.
axes mi-app/api-v1 rename api-legacy

# El nuevo contexto para acceder a él será ahora `mi-app/api-legacy`.
axes mi-app/api-legacy info
```

---

### `repair`

Escanea el sistema de archivos para encontrar y corregir inconsistencias entre el `GlobalIndex` y el estado en disco de tus proyectos.

#### **Funcionalidad**

Actualmente, `repair` puede detectar y ofrecer corregir:

* **Desajustes de Ruta:** Cuando el directorio de un proyecto ha sido movido o renombrado, `repair` detecta que la ruta en el índice está obsoleta y la actualiza a la nueva ubicación correcta.
* *(Las capacidades futuras incluirán la detección de proyectos huérfanos, archivos `project_ref.bin` corruptos, etc.)*

#### **Sintaxis**

```sh
axes [ruta] repair [args...]
```

#### **Argumentos y Banderas**

| Argumento           | Descripción                                                             | Requerido |
| :------------------ | :---------------------------------------------------------------------- | :------- |
| `--recursive`, `-r` | Explora recursivamente todos los proyectos en subdirectorios.             | No       |
| `--depth`, `-d`     | (uint) Profundidad máxima de búsqueda.                                  | No       |
| `--fix`             | Aplica la solución propuesta automáticamente a los errores detectados.   | No       |

#### **Ejemplos de Uso**

```sh
# Escanea el directorio actual y reporta cualquier inconsistencia encontrada.
axes repair

# Escanea el directorio actual y subdirectorios y reporta cualquier inconsistencia encontrada.
axes repair --recursive

# Escanea el directorio actual y subdirectorios y repara automáticamente cualquier inconsistencia encontrada.
axes repair --recursive --fix
```

---

Este documento proporciona una referencia completa de los comandos disponibles. Para aprender a escribir flujos de trabajo potentes, la siguiente lectura recomendada es la **[Guía de `axes.toml` (AXES_TOML_GUIDE.md)](./AXES_TOML_GUIDE.md)**.
