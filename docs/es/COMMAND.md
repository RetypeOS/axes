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
    * Si el **segundo** argumento es una acción del sistema conocida (`info`, `delete`, `start`, etc.), el primer argumento se trata como el contexto del proyecto.
    * *Ejemplo:* `axes mi-app info`

2. **`axes <acción> [args...]`**
    * Si el **primer** argumento es una acción del sistema conocida, `axes` la ejecuta. Esto se usa para comandos globales (`init`, `tree`, `alias`) o cuando se usa un contexto implícito.
    * *Ejemplo (Global):* `axes tree --all`
    * *Ejemplo (Contexto Implícito):* `axes info` (equivalente a `axes . info`)

3. **`axes <ruta_script_virtual> [params...]` (Por Defecto)**
    * Si ninguna de las reglas anteriores coincide, `axes` asume que estás intentando **ejecutar un *script***. Este es un atajo para `axes <ctx> run <script_name>`.
    * *Ejemplo:* `axes build --release` => `run build script del proyecto actual` (`axes run build --release`)
    * *Ejemplo:* `axes api/build --release` => `run build script del hijo 'api' desde el proyecto actual` (`axes api run build --release`)

### **Sistema de Contexto:**

* **Contexto Explícito:** `axes mi-app/api ...`
* **Contexto Implícito (`.`):** Para comandos que requieren un contexto (`info`, `run`, `start`, etc.), si no se proporciona uno, `axes` usa automáticamente `.` (el proyecto en el directorio actual o su primer ancestro).
* **Navegación:** Puedes usar `..` para referirte a un proyecto padre, `*` para el último hijo usado, y atajos como `g!` para el proyecto global.

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

#### **Argumentos y Flags**

| Flag                   | Descripción                                                                              | Requerido |
| :--------------------- | :--------------------------------------------------------------------------------------- | :-------- |
| `--parent <contexto>`  | El contexto del proyecto que será el padre del nuevo. Por defecto es `global`.             | No        |
| `--name <nombre>`      | El nombre del nuevo proyecto. Si no se proporciona, se usa el nombre del directorio.      | No        |
| `--version <ver>`      | La versión inicial para el proyecto (ej. `1.0.0`).                                       | No        |
| `--description <desc>` | Una breve descripción para el proyecto.                                                  | No        |
| *...y otros*           | `init` acepta más *flags* para preconfigurar `[vars]` y `[env]`.                         | No        |

#### **Ejemplos de Uso**

```sh
# Inicializa un proyecto en el directorio actual, y abre el asistente preguntando por los parámetros no indicados.
cd mi-proyecto
axes init

# Inicializa un proyecto especificando su padre por contexto, y el resto de los parámetros se resolverán automáticamente.
cd mi-servicio
axes init --parent monorepo --autosolve

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

#### **Argumentos y Flags**

| Argumento/Flag      | Descripción                                                                                         | Requerido |
| :------------------ | :-------------------------------------------------------------------------------------------------- | :-------- |
| `<ruta>`            | La ruta al proyecto a registrar. Si se omite, se usa el directorio actual.                          | No        |
| `--autosolve`       | Modo no interactivo. La operación fallará si encuentra cualquier conflicto (ej. un UUID existente).| No        |

#### **Ejemplos de Uso**

```sh
# Registra el proyecto en el directorio actual de forma interactiva
axes register

# Registra un proyecto ubicado en otra ruta
axes register ../otro-proyecto-con-axes
```

---

### `unregister`

(Alias: `unreg`)

Elimina uno o más proyectos del índice de `axes`. **Esta acción NO borra ningún archivo**, solo hace que `axes` "olvide" los proyectos.

#### **Sintaxis**

```sh
axes <contexto> unregister [--recursive] [--reparent-to <nuevo_padre>]
```

#### **Comportamiento por Defecto**

Por defecto, `unregister` **no es recursivo**. Solo desregistra el proyecto especificado en `<contexto>`, y sus hijos directos son reasignados al proyecto raíz (generalmente `global`) para evitar romper el grafo.

#### **Argumentos y Flags**

| Flag                     | Descripción                                                                                               | Requerido |
| :----------------------- | :-------------------------------------------------------------------------------------------------------- | :-------- |
| `--recursive`            | Modo recursivo. Desregistra el proyecto especificado Y **todos sus descendientes**. No se reasigna a nadie.      | No        |
| `--reparent-to <padre>` | En lugar de mover los hijos a la raíz, los mueve al `<nuevo_padre>` especificado. No es compatible con `--recursive`. | No        |

#### **Ejemplos de Uso**

```sh
# Desregistra `legacy-service`, sus hijos ahora serán hijos de `global`.
axes mi-app/legacy-service unregister

# Desregistra `prototype` y todos sus sub-proyectos.
axes prototype unregister --recursive

# Desregistra el proyecto "contenedor" `frontend-v1`, moviendo sus hijos a `frontend-v2`.
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

Al igual que `unregister`, `delete` **no es recursivo por defecto** para prevenir pérdidas accidentales de datos. Solo elimina el `.axes/` del proyecto especificado, y sus hijos son reasignados al proyecto raíz.

#### **Argumentos y Flags**

| Flag          | Descripción                                                                                  | Requerido |
| :------------ | :------------------------------------------------------------------------------------------- | :-------- |
| `--recursive` | Modo recursivo. Elimina el `.axes/` del proyecto especificado Y **todos sus descendientes**.   | No        |

#### **Ejemplos de Uso**

```sh
# Elimina la identidad de `old-service`, preservando sus hijos.
axes old-service delete

# Elimina completamente el proyecto `experiment` y todo lo que contiene del ecosistema `axes`.
axes experiment delete --recursive
```

## Inspección y Navegación

Estos comandos ayudan a visualizar y comprender la estructura del árbol de proyectos y la configuración de cada uno. Son operaciones de solo lectura y completamente seguras.

### `tree`

(Alias: `ls`)

Muestra una representación visual del árbol de proyectos registrados, comenzando desde la raíz o un proyecto específico.

#### **Sintaxis**

```sh
axes [<contexto>] tree [-p, --paths] [-u, --uuids] [--all]
```

#### **Comportamiento**

* Si se proporciona `<contexto>`, muestra el subárbol a partir de ese proyecto.
* Si se omite, muestra el árbol completo a partir del proyecto en el directorio actual (`.`). Para ver el árbol completo, usa `axes global tree` o `axes g! tree`.

#### **Argumentos y Flags**

| Argumento/Flag      | Descripción                                                                 | Requerido  |
| :------------------ | :------------------------------------------------------------------------- | :-------- |
| `<contexto>`        | El proyecto desde el que empezar a mostrar el árbol.                       | No        |
| `-p`, `--paths`     | Muestra la ruta física absoluta de cada proyecto.                          | No        |
| `-u`, `--uuids`     | Muestra el UUID único de cada proyecto.                                    | No        |
| `--all`             | Un atajo para mostrar toda la información disponible (`--paths` y `--uuids`).| No        |
| `-d`, `--depth <PROFUNDIDAD>` | Limita la profundidad de visualización del árbol.                                    | No        |
| `--check`           | Comprueba si las rutas de los proyectos existen en el sistema de archivos. | No        |

#### **Ejemplos de Uso**

```sh
# Muestra el árbol de proyectos completo
axes tree

# Muestra el subárbol del monorepo `my-app`
axes my-app tree

# Muestra el árbol completo con rutas y UUIDs, útil para depuración
axes tree --all

# Muestra el árbol del padre del proyecto actual
axes .. tree -p
```

---

### `info`

Muestra un resumen completo de la configuración **final y fusionada** de un proyecto, incluyendo metadatos, *scripts* heredados y variables.

#### **Sintaxis**

```sh
axes [<contexto>] info
```

#### **Argumentos y Flags**

| Argumento    | Descripción                                   | Requerido  |
| :---------- | :-------------------------------------------- | :-------- |
| `<contexto>` | El proyecto cuya información mostrar.         | No        |

#### **Ejemplos de Uso**

```sh
# Muestra la información del proyecto raíz
axes global info

# Muestra la configuración completa del servicio API, incluyendo
# las variables y scripts que ha heredado de `my-app`.
axes my-app/api info
```

La salida de `info` es tu mejor herramienta para depurar por qué un *script* se comporta de cierta manera o de dónde proviene una variable específica.

---

### `alias`

Gestiona atajos (alias) para las rutas de contexto de proyectos. Los alias son globales y te permiten acceder rápidamente a proyectos anidados profundamente.

#### **Sintaxis**

```sh
axes [<ctx>] alias <subcomando> [argumentos...]
```

#### **Subcomandos de `alias`**

| Argumento    | Descripción                                      | Requerido                  |
| :---------- | :----------------------------------------------- | :------------------------- |
| `set`       | Establece un nuevo alias o actualiza uno existente. | `<alias> <contexto_objetivo>` |
| `list`      | Lista todos los alias definidos.                 | Nada                       |
| `remove`    | Elimina un alias.                                | `<alias>`                  |
| `check`     | Verifica todos los alias, reportando enlaces rotos | Nada                       |

**`list`**
(Alias: `ls`)
Muestra una tabla de todos los alias definidos. Este es el subcomando por defecto si no se especifica ninguno.

* **Sintaxis:** `axes alias [list]`

**`set`**
Crea un nuevo alias o actualiza uno existente.

* **Sintaxis:** `axes alias set <nombre_alias> <contexto_objetivo>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo (ej. `api`, `frontend`). No incluyas el `!`.
  * `<contexto_objetivo>`: La ruta completa del proyecto al que apuntará el alias.

**`remove`**
(Alias: `rm`)
Elimina un alias.

* **Sintaxis:** `axes alias rm <nombre_alias>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo a eliminar.

#### **Notas Importantes**

* Los alias se usan añadiendo un `!` al final. Por ejemplo, si creas `axes alias set api my-monorepo/services/main-api`, puedes usarlo con `axes api! info`.
* El alias `g!` es un alias especial por defecto que siempre apunta al proyecto raíz. Se puede modificar o eliminar, pero se mostrará una advertencia.

#### **Ejemplos de Uso**

```sh
# Lista todos los alias
axes alias

# Crea un atajo para un servicio anidado
axes alias set api my-monorepo/services/api-v2

# Usa el nuevo alias
axes api! test

# Elimina un alias
axes alias rm api
```

## Interacción y Ejecución del Proyecto

Estos son los comandos principales que usarás en tu flujo de trabajo diario para ejecutar tareas, iniciar entornos y abrir tus proyectos.

### `run`

Ejecuta un *script* definido en la sección `[scripts]` del `axes.toml` de un proyecto. Este es el comando más potente y versátil de `axes`.

#### **Sintaxis**

```sh
axes [<contexto>] run <nombre_script> [parámetros...]
# O:
axes [<contexto>]/<nombre_script> [parámetros...]
```

#### **Argumentos y Flags**

| Argumento          | Descripción                                                                  | Requerido  |
| :---------------- | :--------------------------------------------------------------------------- | :-------- |
| `<contexto>`       | El contexto del proyecto en el que se ejecutará el *script*. (Implícitamente `.` si se omite). | No       |
| `<nombre_script>`  | El nombre del *script* a ejecutar (la clave bajo la tabla `[scripts]`).     | Sí       |
| `[parámetros...]` | Cualquier argumento adicional que se pasará al *script*.                  | No        |

#### **Funcionalidad Clave**

El comando `run` es orquestado por un potente motor de expansión de texto. Dentro de tus *scripts*, puedes usar una sintaxis especial `<...>` para:

* **Incluir variables:** `<vars::mi_variable>`
* **Componer otros *scripts*:** `<scripts::otro_script>`
* **Ejecutar comandos y sustituir su salida:** `<run('git rev-parse --short HEAD')>`
* **Pasar parámetros de la CLI de forma estructurada:** `<params::0>`, `<params::fl(map='--flag', default='some', required)>`, `<params>`

> **Nota:** El sistema de *scripting* y parámetros es la característica más profunda de `axes`. Para una guía completa con ejemplos, consulta el **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

#### **Ejemplos de Uso**

```sh
# Ejecuta el script 'build' en el proyecto actual (usando atajo)
axes build

# Ejecuta el script 'build' en el proyecto `my-app/frontend`
axes my-app/frontend run build

# Ejecuta el script 'test' y le pasa un parámetro
# (asumiendo que `test` usa `<params>` o `<params::0>`)
axes my-app/api/test tests/unit/test_auth.py

# Ejecuta un script pasándole un flag
# (asumiendo que `deploy` usa `<params::production>`)
axes my-app/deploy --production
```

---

### `start`

Inicia una sesión de *shell* interactiva y persistente dentro del contexto de un proyecto. Es la herramienta ideal para un trabajo enfocado.

#### **Sintaxis**

```sh
axes [<contexto>] start [parámetros...]
```

#### **Argumentos y Flags**

| Argumento          | Descripción                                                                         | Requerido |
| :---------------- | :---------------------------------------------------------------------------------- | :-------- |
| `<contexto>`       | El proyecto en el que iniciar la sesión. (Implícitamente `.` si se omite).             | No       |
| `[parámetros...]` | Cualquier argumento adicional que se pasará a los *hooks* `at_start` y `at_exit`. | No       |

#### **Comportamiento de la Sesión**

Al ejecutar `start`, `axes` hace lo siguiente:

1. **Resuelve y Valida Parámetros:** `axes` analiza los `[parámetros...]` proporcionados y los valida contra las definiciones `<params::...>` encontradas en los *hooks* de `at_start` y `at_exit`.
2. **Ejecuta el Hook `at_start`:** Se ejecuta el *script* `at_start`, inyectando los parámetros resueltos.
3. **Inicia el Shell:** Se lanza el *shell* interactivo con todas las variables de `[env]` inyectadas.

Una vez dentro, puedes ejecutar comandos `axes` sin especificar el contexto. Al salir de la sesión con `exit`, se ejecuta el *hook* `at_exit`, que también recibe los mismos `[parámetros...]` resueltos al inicio de la sesión.

#### **Ejemplos de Uso**

```sh
# Inicia una sesión simple en el servicio de API
axes my-app/api start

# Asumiendo un `at_start` como: "docker-compose up -d <params::service>"
# Inicia una sesión y especifica qué servicio levantar
axes my-app/api start --service web
```

---

### `open`

Abre el directorio raíz de un proyecto utilizando una aplicación preconfigurada.

#### **Sintaxis**

```sh
axes [<contexto>] open [<clave_app>] [parámetros...]
```

#### **Argumentos y Flags**

| Argumento         | Descripción                                                                          | Requerido |
| :---------------- | :----------------------------------------------------------------------------------- | :-------- |
| `<contexto>`       | El proyecto a abrir. (Implícitamente `.` si se omite).                               | No       |
| `[<clave_app>]`     | La clave de la aplicación a usar (ej. `code`). Si se omite, se usa la clave `default`. | No       |
| `[parámetros...]` | Cualquier argumento adicional que se pasará al script de `clave_app`. | No       |

#### **Configuración**

Las aplicaciones se definen en la sección `[options.open_with]` de tu `axes.toml`. Cada entrada es un **script completo** que puede ser una cadena, una secuencia o una tabla con una descripción.

```toml
[options.open_with]
# Atajo simple de cadena
edit = "code \"<path>\""

# Atajo con descripción y que acepta parámetros
terminal = { desc = "Abre un terminal en una subcarpeta.", run = "wt -d \"<path>/<params::0(default='.')>\"" }

# Establece la acción por defecto
default = "edit"
```

#### **Ejemplos de Uso**

```sh
# Abre el proyecto actual con la aplicación por defecto (implícitamente axes . open)
axes open

# Abre explícitamente el proyecto `my-app/api` en el explorador de archivos
# (Asumiendo que existe una clave 'files')
axes my-app/api open files

# Usa el atajo 'terminal' y pasa un parámetro para abrir en el subdirectorio 'src'
axes my-app/frontend open terminal src
```

## Refactorización del Árbol de Proyectos

Estos comandos permiten modificar la estructura de tu ecosistema `axes`, cambiando las relaciones entre proyectos y sus nombres. Son operaciones potentes que actualizan el índice global de `axes`.

### `link`

Cambia el padre de un proyecto existente, moviéndolo a una nueva ubicación en el árbol lógico. Esta operación es puramente estructural y no mueve ningún archivo en tu disco.

#### **Sintaxis**

```sh
axes <contexto_hijo> link <contexto_nuevo_padre>
```

#### **Argumentos y Flags**

| Argumento                   | Descripción                                       | Requerido  |
| :-------------------------- | :------------------------------------------------ | :-------- |
| `<contexto_hijo>`           | El proyecto que deseas mover.                     | Sí       |
| `<contexto_nuevo_padre>`     | El proyecto que se convertirá en su nuevo padre.  | Sí       |

#### **Validaciones de Seguridad**

`link` es una operación segura. `axes` evitará cualquier acción que pueda corromper el grafo de proyectos, fallando con un error claro si intentas:

* **Crear un ciclo:** Mover un proyecto para que se convierta en su propio descendiente (ej. `axes A link A/B`).
* **Crear una colisión de nombre:** Mover un proyecto a un nuevo padre que ya tiene un hijo con el mismo nombre.

#### **Ejemplos de Uso**

```sh
# `legacy-service` era hijo de `global`, ahora será hijo de `monorepo-v2`.
axes legacy-service link monorepo-v2

# Mueve `admin-panel` para que sea hijo del servicio `api` en lugar de `frontend`.
axes mi-app/frontend/admin-panel link mi-app/api
```

---

### `rename`

Cambia el nombre de un proyecto. Este es el nombre utilizado en las rutas de contexto, no el nombre del directorio en el disco.

#### **Sintaxis**

```sh
axes <contexto> rename <nuevo_nombre>
```

#### **Argumentos y Flags**

| Argumento          | Descripción                                   | Requerido  |
| :---------------- | :-------------------------------------------- | :-------- |
| `<contexto>`       | El proyecto que deseas renombrar.               | Sí       |
| `<nuevo_nombre>`   | El nuevo nombre para el proyecto.                 | Sí       |

#### **Reglas de Nomenclatura**

El `<nuevo_nombre>` debe seguir ciertas reglas para asegurar la estabilidad:

* **No puede contener espacios** ni caracteres de ruta (`/`, `\`).
* **No puede ser un nombre reservado** para navegación (ej. `.` , `..`, `*`).

`axes` también te advertirá si intentas usar nombres que, aunque válidos, no son recomendables (ej. si empiezan con `-`). Renombrar el proyecto raíz `global` está permitido pero requerirá una confirmación adicional debido a su importancia.

#### **Ejemplos de Uso**

```sh
# Renombra un proyecto de `api-v1` a `api-legacy`.
axes mi-app/api-v1 rename api-legacy

# El nuevo contexto para acceder a él será ahora `my-app/api-legacy`.
axes my-app/api-legacy info
```

---

### `repair`

Reporta errores en el sistema de proyectos y/o en los archivos de proyecto, y ofrece repararlos automáticamente.

#### **Sintaxis**

```sh
axes [ruta] repair [args...]
```

#### **Argumentos y Flags**

| Argumento            | Descripción                                                             | Requerido |
| :------------------ | :---------------------------------------------------------------------- | :------- |
| `--recursive`, `-r` | Explora recursivamente todos los proyectos en los subdirectorios        | No       |
| `--depth`, `-d`     | (uint) Profundidad máxima de búsqueda.                                  | No       |
| `--fix`             | Aplica la solución propuesta automáticamente a los errores detectados.   | No       |

#### **Ejemplos de Uso**

```sh
# Escanea el directorio actual e informa de cualquier inconsistencia encontrada.
axes repair

# Escanea el directorio actual y los subdirectorios e informa de cualquier inconsistencia encontrada.
axes repair --recursive

# Escanea el directorio actual y los subdirectorios y repara automáticamente cualquier inconsistencia encontrada.
axes repair --recursive --fix
```

---

Este documento proporciona una referencia completa de los comandos disponibles. Para aprender a escribir flujos de trabajo potentes, la siguiente lectura recomendada es la **[Guía de `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.
