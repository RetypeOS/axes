<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../COMMAND.md">English</a> |
  <a href="./COMMAND.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../COMMAND.md), que es la fuente canónica de la documentación.

# Referencia Completa de Comandos

Este documento es la guía de referencia definitiva para cada comando disponible en la CLI de `axes`. Para un tutorial guiado, consulta la [**Guía de Inicio (`GETTING_STARTED.md`)**](./GETTING_STARTED.md).

## Sintaxis General y Atajos

`axes` utiliza una sintaxis flexible para la mayoría de sus comandos, permitiéndote priorizar la acción o el contexto según tu preferencia.

```sh
# Ambas formas son generalmente válidas:
axes <acción> <contexto> [argumentos...]
axes <contexto> <acción> [argumentos...]
```

> **Navegación por Sistema de Archivos:** Los contextos especiales `.` y `..` te permiten interactuar con proyectos basados en tu ubicación actual en el terminal, de forma similar a `cd`. `axes . info` muestra la información del proyecto del directorio actual (o su primer ancestro), mientras que `axes .. info` muestra la del directorio padre o superior.

Adicionalmente, `axes` ofrece dos atajos importantes para acelerar tu flujo de trabajo:

* **Atajo para `start`:** Si solo proporcionas un contexto, `axes` asume que quieres iniciar una sesión.

    ```sh
    # Esto es equivalente a `axes mi-app/api start`
    axes mi-app/api
    ```

* **Atajo para `run`:** Si el segundo argumento no es una acción del sistema, `axes` asume que es el nombre de un script que quieres ejecutar.

    ```sh
    # Esto es equivalente a `axes mi-app/api run build`
    axes mi-app/api build
    ```

---

## Gestión del Ciclo de Vida del Proyecto

Estos comandos se utilizan para crear, registrar, y eliminar proyectos del índice de `axes`.

### `init`

(Alias: Ninguno)

Inicializa `axes` en el directorio actual, creando una estructura `.axes/` con un `axes.toml` por defecto y registrando el proyecto.

#### **Sintaxis**

```sh
axes init [--parent <contexto_padre>] [--name <nombre>] [--version <ver>] [--description <desc>]
```

#### **Argumentos y Flags**

| Flag                   | Descripción                                                                              | Requerido |
| :--------------------- | :--------------------------------------------------------------------------------------- | :-------- |
| `--parent <contexto>`  | El contexto del proyecto que será el padre del nuevo. Por defecto es `global`.           | No        |
| `--name <nombre>`      | El nombre para el nuevo proyecto. Si no se proporciona, se usa el nombre del directorio. | No        |
| `--version <ver>`      | La versión inicial para el proyecto (ej. `1.0.0`).                                       | No        |
| `--description <desc>` | Una breve descripción para el proyecto.                                                  | No        |
| *...y otros*           | `init` acepta más flags para pre-configurar `[vars]` y `[env]`.                          | No        |

#### **Ejemplos de Uso**

```sh
# Inicializa un proyecto en el directorio actual, e inicia el asistente preguntando por los parámetros no indicados.
cd mi-proyecto
axes init

# Inicializa un proyecto especificando su padre por contexto, y el resto de parámetros se resolveran de forma automática.
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

#### **Argumentos y Flags**

| Argumento/Flag      | Descripción                                                                                            | Requerido |
| :------------------ | :----------------------------------------------------------------------------------------------------- | :-------- |
| `<ruta>`            | La ruta al proyecto que se quiere registrar. Si se omite, se usa el directorio actual.                 | No        |
| `--autosolve`       | Modo no interactivo. La operación fallará si encuentra cualquier conflicto (ej. un UUID ya existente). | No        |

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

Elimina uno o más proyectos del índice de `axes`. **Esta acción NO borra ningún archivo**, solo hace que `axes` "olvide" los proyectos.

#### **Sintaxis**

```sh
axes <contexto> unregister [--recursive] [--reparent-to <nuevo_padre>]
```

#### **Comportamiento por Defecto**

Por defecto, `unregister` **no es recursivo**. Solo desregistra el proyecto especificado en `<contexto>`, y sus hijos directos son re-parentados al proyecto raíz (normalmente `global`) para evitar romper el grafo.

#### **Argumentos y Flags**

| Flag                     | Descripción                                                                                                | Requerido |
| :----------------------- | :--------------------------------------------------------------------------------------------------------- | :-------- |
| `--recursive`            | Modo recursivo. Desregistra el proyecto especificado Y **todos sus descendientes**. No hay re-parenting.   | No        |
| `--reparent-to <padre>`  | En lugar de mover los hijos a la raíz, los mueve al `<nuevo_padre>` especificado. No es compatible con `--recursive`. | No        |

#### **Ejemplos de Uso**

```sh
# Desregistra `servicio-legado`, sus hijos ahora serán hijos de `global`.
axes mi-app/servicio-legado unregister

# Desregistra `prototipo` y todos sus sub-proyectos.
axes prototipo unregister --recursive

# Desregistra el "contenedor" `frontend-v1`, moviendo sus hijos a `frontend-v2`.
axes frontend-v1 unregister --reparent-to frontend-v2
```

---

### `delete`

(Alias: `del`)

☢️ **ACCIÓN DESTRUCTIVA.** Elimina el directorio `.axes/` de un proyecto (y opcionalmente de sus hijos) Y lo desregistra del índice.

#### **Sintaxis**

```sh
axes <contexto> delete [--recursive]
```

#### **Comportamiento por Defecto**

Al igual que `unregister`, `delete` **no es recursivo por defecto** para prevenir la pérdida accidental de datos. Solo elimina el `.axes/` del proyecto especificado, y sus hijos son re-parentados al proyecto raíz.

#### **Argumentos y Flags**

| Flag          | Descripción                                                                                     | Requerido |
| :------------ | :---------------------------------------------------------------------------------------------- | :-------- |
| `--recursive` | Modo recursivo. Elimina el `.axes/` del proyecto especificado Y de **todos sus descendientes**. | No        |

#### **Ejemplos de Uso**

```sh
# Elimina la identidad de `viejo-servicio`, preservando a sus hijos.
axes viejo-servicio delete

# Elimina completamente del ecosistema `axes` el proyecto `experimento` y todo lo que contiene.
axes experimento delete --recursive
```

## Inspección y Navegación

Estos comandos te ayudan a visualizar y entender la estructura de tu árbol de proyectos y la configuración de cada uno. Son operaciones de solo lectura y completamente seguras.

### `tree`

(Alias: `ls`)

Muestra una representación visual del árbol de proyectos registrados, comenzando desde la raíz o desde un proyecto específico.

#### **Sintaxis**

```sh
axes tree [<contexto>] [-p, --paths] [-u, --uuids] [--all]
```

#### **Comportamiento**

* Si se ejecuta sin `<contexto>`, muestra el árbol completo desde el proyecto raíz.
* Si se proporciona un `<contexto>`, muestra solo ese proyecto y sus descendientes.

#### **Argumentos y Flags**

| Argumento/Flag      | Descripción                                                                   | Requerido |
| :------------------ | :---------------------------------------------------------------------------- | :-------- |
| `<contexto>`        | El proyecto desde el cual comenzar a mostrar el árbol.                        | No        |
| `-p`, `--paths`     | Muestra la ruta física absoluta de cada proyecto.                             | No        |
| `-u`, `--uuids`     | Muestra el UUID único de cada proyecto.                                       | No        |
| `--all`             | Un atajo para mostrar toda la información disponible (`--paths` y `--uuids`). | No        |

#### **Ejemplos de Uso**

```sh
# Muestra el árbol de proyectos completo
axes tree

# Muestra el sub-árbol del monorepo `mi-app`
axes tree mi-app

# Muestra el árbol completo con rutas y UUIDs, útil para depuración
axes tree --all

# Muestra el árbol del proyecto padre al actual
axes .. tree -p
```

---

### `info`

Muestra un resumen completo de la configuración **final y fusionada** de un proyecto, incluyendo metadatos, scripts heredados y variables.

#### **Sintaxis**

```sh
axes <contexto> info
```

#### **Argumentos y Flags**

| Argumento    | Descripción                                  | Requerido |
| :----------- | :------------------------------------------- | :-------- |
| `<contexto>` | El proyecto del cual mostrar la información. | Sí        |

#### **Ejemplos de Uso**

```sh
# Muestra la información del proyecto raíz
axes global info

# Muestra la configuración completa del servicio de API, incluyendo
# las variables y scripts que ha heredado de `mi-app`.
axes mi-app/api info
```

La salida de `info` es tu mejor herramienta para depurar por qué un script se comporta de cierta manera o de dónde proviene una variable específica.

---

### `alias`

Gestiona atajos (aliases) para las rutas de contexto de tus proyectos. Los alias son globales y te permiten acceder rápidamente a proyectos anidados profundamente.

#### **Sintaxis**

```sh
axes alias <subcomando> [argumentos...]
```

#### **Subcomandos de `alias`**

**`list`**
(Alias: `ls`)
Muestra una tabla de todos los alias definidos. Este es el subcomando por defecto si no se especifica ninguno.

* **Sintaxis:** `axes alias [list]`

**`set`**
Crea un nuevo alias o actualiza uno existente.

* **Sintaxis:** `axes alias set <nombre_alias> <contexto_destino>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo (ej. `api`, `frontend`). No incluyas el `!`.
  * `<contexto_destino>`: La ruta completa del proyecto al que apuntará el alias.

**`remove`**
(Alias: `rm`)
Elimina un alias.

* **Sintaxis:** `axes alias rm <nombre_alias>`
* **Argumentos:**
  * `<nombre_alias>`: El nombre del atajo a eliminar.

#### **Notas Importantes**

* Los alias se usan añadiendo un `!` al final. Por ejemplo, si creas `axes alias set api mi-app/backend/api-principal`, puedes usarlo con `axes api! info`.
* El alias `g!` es un alias especial por defecto que siempre apunta al proyecto raíz. Puede ser modificado o eliminado, pero se mostrará una advertencia.

#### **Ejemplos de Uso**

```sh
# Listar todos los alias
axes alias

# Crear un atajo para un servicio anidado
axes alias set api mi-monorepo/servicios/api-v2

# Usar el nuevo alias
axes api! test

# Eliminar un alias
axes alias rm api
```

## Interacción y Ejecución de Proyectos

Estos son los comandos principales que usarás en tu flujo de trabajo diario para ejecutar tareas, iniciar entornos y abrir tus proyectos.

### `run`

Ejecuta un script definido en la sección `[scripts]` del `axes.toml` de un proyecto. Este es el comando más potente y versátil de `axes`.

#### **Sintaxis**

```sh
axes <contexto> run <nombre_script> [parámetros...]
```

* **Atajo:** `axes <contexto> <nombre_script> [parámetros...]`

#### **Argumentos y Flags**

| Argumento         | Descripción                                                                   | Requerido |
| :---------------- | :---------------------------------------------------------------------------- | :-------- |
| `<contexto>`      | El proyecto en cuyo contexto se ejecutará el script.                          | Sí        |
| `<nombre_script>` | El nombre del script a ejecutar (la clave bajo la tabla `[scripts]`).         | Sí        |
| `[parámetros...]` | Cualquier argumento adicional que se pasará al script.                        | No        |

#### **Funcionalidad Clave**

El comando `run` es orquestado por un potente motor de expansión de texto. Dentro de tus scripts, puedes usar una sintaxis especial `<axes::...>` para:

* **Incluir variables:** `<axes::vars::mi_variable>`
* **Componer otros scripts:** `<axes::scripts::otro_script>`
* **Ejecutar comandos y sustituir su salida:** `<axes::run::git rev-parse --short HEAD>`
* **Pasar parámetros de la CLI de forma estructurada:** `<axes::params::0>`, `<axes::params::flag='--flag'>`, `<axes::params>`

> **Nota:** El sistema de scripting y parámetros es la característica más profunda de `axes`. Para una guía completa con ejemplos, consulta **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

#### **Ejemplos de Uso**

```sh
# Ejecuta el script 'build' en el proyecto `mi-app/frontend`
axes mi-app/frontend run build

# Usa el atajo para hacer lo mismo
axes mi-app/frontend build

# Ejecuta el script 'test' y le pasa un parámetro
# (asumiendo que `test` usa `<axes::params>` o `<axes::params::0>`)
axes mi-app/api test tests/unit/test_auth.py

# Ejecuta un script pasándole un flag
# (asumiendo que `deploy` usa `<axes::params::production='--prod'>`)
axes mi-app deploy --production
```

---

### `start`

Inicia una sesión de shell interactiva y persistente dentro del contexto de un proyecto. Es la herramienta ideal para un trabajo enfocado.

#### **Sintaxis**

```sh
axes <contexto> start [parámetros...]
```

* **Atajo:** `axes <contexto> [parámetros...]`

#### **Argumentos y Flags**

| Argumento         | Descripción                                                                     | Requerido |
| :---------------- | :------------------------------------------------------------------------------ | :-------- |
| `<contexto>`      | El proyecto en el cual iniciar la sesión.                                       | Sí        |
| `[parámetros...]` | Cualquier argumento adicional que se pasará a los hooks `at_start` y `at_exit`. | No        |

#### **Comportamiento de la Sesión**

Al ejecutar `start`, `axes` hace lo siguiente:

1. **Resuelve y Valida Parámetros:** `axes` analiza los `[parámetros...]` proporcionados y los valida contra las definiciones `<axes::params::...>` encontradas en los hooks `at_start` y `at_exit`.
2. **Ejecuta el Hook `at_start`:** Se ejecuta el script `at_start`, inyectando los parámetros resueltos.
3. **Inicia la Shell:** Se lanza la shell interactiva con todas las variables de `[env]` inyectadas.

Una vez dentro, puedes ejecutar comandos de `axes` sin especificar el contexto. Al salir de la sesión con `exit`, se ejecuta el hook `at_exit`, que también recibe los mismos `[parámetros...]` resueltos al inicio de la sesión.

#### **Ejemplos de Uso**

```sh
# Inicia una sesión simple en el servicio de API
axes mi-app/api

# Suponiendo un `at_start` como: "docker-compose up -d <axes::params::service>"
# Inicia una sesión y especifica qué servicio levantar
axes mi-app/api start --service web
```

---

### `open`

Abre el directorio raíz de un proyecto usando una aplicación pre-configurada.

#### **Sintaxis**

```sh
axes <contexto> open [<app_key>] [parámetros...]
```

#### **Argumentos y Flags**

| Argumento         | Descripción                                                                              | Requerido |
| :---------------- | :--------------------------------------------------------------------------------------- | :-------- |
| `<contexto>`      | El proyecto que se quiere abrir.                                                         | Sí        |
| `[<app_key>]`     | La clave de la aplicación a usar (ej. `code`). Si se omite, se usa la clave `default`.   | No        |
| `[parámetros...]` | (Nuevo en v0.1.8) Cualquier argumento adicional que se pasará al script de la `app_key`. | No        |

#### **Configuración**

Las aplicaciones se definen en la sección `[options.open_with]` de tu `axes.toml`. Desde la v0.1.8, cada entrada es un **script completo** que puede ser un string, una secuencia o una tabla con descripción.

```toml
[options.open_with]
# Atajo simple como string
edit = "code \"<axes::path>\""

# Atajo con descripción y que acepta parámetros
terminal = { desc = "Abre una terminal en una subcarpeta.", run = "wt -d \"<axes::path>/<axes::params::0(default='.')>\"" }

# Establece la acción por defecto
default = "edit"
```

#### **Ejemplos de Uso**

```sh
# Abre el proyecto `mi-app` con la aplicación por defecto ('edit' en nuestro ejemplo)
axes mi-app open

# Abre explícitamente el proyecto `mi-app/api` en el explorador de archivos
# (Asumiendo que hay una clave 'files' definida)
axes mi-app/api open files

# Usa el atajo 'terminal' y le pasa un parámetro para abrir en el subdirectorio 'src'
axes mi-app/frontend open terminal src
```

## Refactorización del Árbol de Proyectos

Estos comandos te permiten modificar la estructura de tu ecosistema `axes`, cambiando las relaciones entre proyectos y sus nombres. Son operaciones potentes que actualizan el índice global de `axes`.

### `link`

Cambia el padre de un proyecto existente, moviéndolo a una nueva ubicación en el árbol lógico. Esta operación es puramente estructural y no mueve ningún archivo en tu disco.

#### **Sintaxis**

```sh
axes <contexto_hijo> link <contexto_nuevo_padre>
```

#### **Argumentos y Flags**

| Argumento                 | Descripción                                      | Requerido |
| :------------------------ | :----------------------------------------------- | :-------- |
| `<contexto_hijo>`         | El proyecto que quieres mover.                   | Sí        |
| `<contexto_nuevo_padre>`  | El proyecto que se convertirá en su nuevo padre. | Sí        |

#### **Validaciones de Seguridad**

`link` es una operación segura. `axes` prevendrá cualquier acción que pueda corromper el grafo de proyectos, fallando con un error claro si intentas:

* **Crear un ciclo:** Mover un proyecto para que se convierta en su propio descendiente (ej. `axes A link A/B`).
* **Crear una colisión de nombres:** Mover un proyecto a un nuevo padre que ya tiene un hijo con ese mismo nombre.

#### **Ejemplos de Uso**

```sh
# `servicio-legacy` era un hijo de `global`, ahora será hijo de `monorepo-v2`.
axes servicio-legacy link monorepo-v2

# Mueve el `panel-admin` para que sea un hijo del servicio `api` en lugar de `frontend`.
axes mi-app/frontend/panel-admin link mi-app/api
```

---

### `rename`

Cambia el nombre de un proyecto. Este es el nombre utilizado en las rutas de contexto, no el nombre del directorio en el disco.

#### **Sintaxis**

```sh
axes <contexto> rename <nuevo_nombre>
```

#### **Argumentos y Flags**

| Argumento         | Descripción                           | Requerido |
| :---------------- | :------------------------------------ | :-------- |
| `<contexto>`      | El proyecto que quieres renombrar.    | Sí        |
| `<nuevo_nombre>`  | El nuevo nombre para el proyecto.     | Sí        |

#### **Reglas de Nombramiento**

El `<nuevo_nombre>` debe seguir ciertas reglas para garantizar la estabilidad:

* **No puede contener espacios** ni caracteres de ruta (`/`, `\`).
* **No puede ser un nombre reservado** para la navegación (ej. `.` , `..`, `*`).

`axes` también te advertirá si intentas usar nombres que, aunque válidos, no son recomendables (ej. que empiecen con `-`). Renombrar el proyecto raíz `global` está permitido, pero requerirá una confirmación adicional debido a su importancia.

#### **Ejemplos de Uso**

```sh
# Cambia el nombre de un proyecto de `api-v1` a `api-legacy`.
axes mi-app/api-v1 rename api-legacy

# El nuevo contexto para acceder a él será ahora `mi-app/api-legacy`.
axes mi-app/api-legacy info
```

---
Este documento proporciona una referencia completa de los comandos disponibles. Para aprender a escribir flujos de trabajo potentes, la siguiente lectura recomendada es la **[Guía de `axes.toml`](./AXES_TOML_GUIDE.md)**.
