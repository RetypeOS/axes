
<p align="center">
  <img src="../../logo.png" alt="axes Logo" width="200">
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="https://github.com/retypeos/axes/releases"><img src="https://img.shields.io/badge/version-v0.2.1--beta-blue" alt="Version"></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../README.md">English</a> •
  <a href="./README.md">Español</a>
</p>

<h1 align="center">axes: El Director de Orquesta para Tu Caos de Desarrollo</h1>

<p align="center">
  <strong>El poder de un orquestador complejo. La velocidad de un simple ejecutor.</strong>
</p>

> **Nota:** Esta traducción es mantenida principalmente por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../README.md), que es la fuente canónica de la documentación.

---

## Tu Flujo de Trabajo es un Desorden. Lo Hemos Arreglado

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **Tú, 3 semanas después:** *«Espera... ¿el comando para los tests era `npm test`, `pytest`, `cargo test` o `go test ./...`?»*

Esa duda, esa carga cognitiva, es fricción. Mata tu flujo. Los ejecutores de tareas simples te dan atajos. **`axes` te da un lenguaje universal.**

`axes` es un orquestador de flujos de trabajo de alto rendimiento escrito en Rust. No es solo otro gestor de tareas; es un **lenguaje de comandos** que estandariza cómo construyes, pruebas y ejecutas cualquier proyecto, desde un script simple hasta un monorepo políglota complejo. Reemplaza `Makefile`s dispersos, scripts de `package.json` y scripts de shell frágiles con una interfaz única, consistente y extremadamente rápida.

`axes` es el director de orquesta que conoce cada instrumento de tu arsenal, convirtiendo tu colección caótica de herramientas en una sinfonía.

### ¿Quién Dijo que Tenías que Elegir Entre Potencia y Velocidad?

Durante años, la elección ha sido una falsa dicotomía:

- **Ejecutores Simples (`just`, `make`):** Rápidos de iniciar, pero limitados. Son gestores de alias glorificados, carentes de jerarquía, parametrización y capacidades de orquestación reales.
- **Orquestadores Complejos (`Bazel`, `Gradle`):** Increíblemente potentes, pero notoriamente lentos, complejos de configurar y rígidos. El coste de inicio es un lastre constante para la productividad.

**`axes` rompe este compromiso.** Ofrecemos las capacidades avanzadas de orquestación de sistemas complejos a una velocidad que rivaliza—y a menudo supera—a los ejecutores más simples.

| Herramienta | Ejecución de Script en Caliente | Características de Orquestación |
|:---|:---:|:---:|
| **`axes --version`** | **19.6 ± 1.8 ms** | **1.00** |
| `just --version` | 24.4 ± 3.5 ms | 1.25x más lento |
| `task --version` | 69.0 ± 9.0 ms | 3.52x más lento |
| | | |
| **`axes <script>`** | **41.8 ± 1.9 ms** | **1.00** |
| `just <script>` | 44.7 ± 4.0 ms | 1.07x más lento |
| `task <script>` | 79.9 ± 9.3 ms | 1.91x más lento |

*Benchmarks ejecutados en una máquina de desarrollo estándar (Windows 11, Intel Core i7, 16GB RAM, SSD NVMe) usando `hyperfine`. Cada comando se ejecutó 50 veces después de un calentamiento de 5 ejecuciones.*

Esto no es magia; es ingeniería obsesiva.

- **Carga de Configuración Perezosa y Paralela:** `axes` carga inteligentemente solo la configuración que necesita, en paralelo, minimizando la E/S de inicio.
- **Caché de AST Pre-compilado:** La primera vez que ejecutas un script, `axes` actúa como un compilador Just-in-Time. Parsea tu `axes.toml`, resuelve toda la herencia y composición, y lo compila en un **Árbol de Sintaxis Abstracta (AST)** altamente optimizado.
- **Ejecuciones en Caliente Instantáneas:** Cada ejecución posterior omite por completo el costoso trabajo de parseo de texto. Deserializa el AST pre-compilado del caché binario—una operación órdenes de magnitud más rápida—y lo ejecuta.

**El resultado: pagas el coste de la orquestación una sola vez. Obtienes la velocidad de un ejecutor simple cada vez después.**

- ⚙️ **[Inmersión Profunda en la Arquitectura de `axes` (`TECNICAL.md`)](./TECNICAL.md):** Si te interesa investigar más a fondo la arquitectura de `axes`, este es el mejor lugar para empezar.

---

### La Filosofía `axes`: Más que un Ejecutor de Tareas

`axes` se construye sobre una base que las herramientas simples ignoran.

#### 1. Orquestación, No Solo Ejecución

Los proyectos tienen relaciones. `axes` te permite organizarlos en un árbol lógico (`app/api`, `app/web`). Los hijos heredan automáticamente scripts, variables y configuraciones de entorno de sus padres, que pueden anular según sea necesario. **Define una vez, usa en todas partes.**

```sh
# Un script definido en la configuración 'global' está disponible en todas partes.
$ axes my-app/api/db migrate

# El script 'build' en 'my-app/api' puede llamar al script 'build' de su padre.
$ axes my-app/api build
```

#### 2. Ergonomía, No Solo Atajos

Tus scripts se convierten en aplicaciones de línea de comandos de primera clase, con parámetros tipados, valores por defecto y validación, todo ello sin escribir una sola línea de código de análisis repetitivo.

```toml
# en .axes/axes.toml
[scripts]
deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
#                                              ^-- Un parámetro posicional
#                                                 con un valor por defecto.
```

```sh
axes deploy production  # Se ejecuta con env='production'
axes deploy             # Se ejecuta con env='staging'
```

#### 3. Robustez por Diseño

`axes` identifica los proyectos mediante un **UUID** inmutable, no una ruta de archivo frágil. Renombra o mueve tus directorios de proyecto libremente: el índice de `axes` se autocurará y nunca perderá la pista de tus proyectos. Esto hace que la refactorización de grandes monorepos sea trivial y segura.

---

### Instalación (30 Segundos para un Mejor Flujo de Trabajo)

`axes` es un único binario sin dependencias escrito en Rust.

1. Ve a la [**página de Releases de `axes` en GitHub**](https://github.com/retypeos/axes/releases).
2. Descarga el archivo para tu sistema operativo (`windows-x86_64`, [ `linux-x86_64`, `macos-x86_64` ](No disponibles aún, deberá compilarlos en su dispositivo para probarlo linux o macos)).
3. Descomprímelo y mueve el ejecutable `axes` a un directorio en tu `PATH` (ej. `/usr/local/bin`, `C:\Windows\System32`).
4. Abre una **nueva terminal** y verifica la instalación con `axes --version`.

---

### `axes` en Acción: Un Vistazo al Poder

#### 1. Comandos Universales y Conscientes del Contexto

Ejecuta un script en el contexto del directorio actual. La sintaxis es simple y predecible.

```sh
# Ejecuta el script 'build' definido en el axes.toml más cercano
axes build --release
```

#### 2. Flujos de Trabajo Multiplataforma sin Esfuerzo

Define un comando una vez. Funciona para todo tu equipo, en cualquier SO.

```toml
[vars]
host = "http://localhost:8080"

[scripts.browse]
desc    = "Abre la documentación local en el navegador."
windows = "start <axes::vars::host>"
macos   = "open <axes::vars::host>"
linux   = "xdg-open <axes::vars::host>"

```

#### 3. Composición Dinámica en Tiempo Real

Ejecuta comandos y usa su salida como variable al instante.

```toml
[scripts]
# Etiqueta una imagen Docker con el hash corto de git actual
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Sesiones de Enfoque Inmersivo

Sumérgete en un sub-proyecto. `axes` configura y desmantela tu entorno por ti, automáticamente.

```toml
# en my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Se ejecuta al entrar en la sesión
at_exit  = "docker-compose down"       # Se ejecuta al salir de la sesión
```

```sh
$ axes my-app/api start  # Inicia una sesión. `at_start` se ejecuta automáticamente.

(axes: my-app/api) $ axes test  # Ahora estás "dentro" de my-app/api.
(axes: my-app/api) $ exit       # `at_exit` se ejecuta automáticamente.
```

**Tu entorno de desarrollo, bajo demanda.**

---

### ¿Listo para Dirigir tu Propia Orquesta?

La fricción que sientes cada día no es un requisito. Es un problema con una solución. `axes` es esa solución.

- ➡️ **[Guía de Inicio Rápido (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Construye tu primer monorepo orquestado en 15 minutos.
- 📖 **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La referencia definitiva de cada característica y sintaxis.
- ⌨️ **[Referencia de Comandos (`COMMAND.md`)](./COMMAND.md):** Una guía completa de cada comando incorporado en la CLI (`init`, `register`, `tree`, etc.).

### Únete a la Revolución del Flujo de Trabajo

`axes` es más que una herramienta; es un movimiento para devolver el control y la coherencia al desarrollo. Tu voz es crucial.

- **Encuentra un Bug o tienes una Idea Genial:** [**Abre un Issue**](https://github.com/retypeos/axes/issues)
- **Quieres Contribuir con Código:** ¡Los Pull Requests son siempre bienvenidos! Consulta nuestras [Guías de Contribución](./CONTRIBUTING.md).

**Instala `axes` hoy. Deja de recordar comandos. Empieza a construir.**
