
<p align="center">
  <img src="../../logo.png" alt="axes Logo" width="200">
</p>

<h1 align="center">axes: El Director de Orquesta para Tu Caos de Desarrollo</h1>

<p align="center">
  <strong>El poder de un orquestador complejo. La velocidad de un simple ejecutor.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="https://github.com/retypeos/axes/releases"><img src="https://img.shields.io/badge/version-v0.2.1--beta-blue" alt="Version"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
  <a href="../../LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../README.md">English</a> •
  <a href="./README.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida principalmente por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../README.md), que es la fuente canónica de la documentación.

---

## Tu Flujo de Trabajo es un Desorden. Lo Hemos Arreglado

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **Tú, 3 semanas después:** *«Espera... ¿el comando para los tests era `npm test`, `pytest`, `cargo test` o `go test ./...`?»*

Esa duda, esa carga cognitiva, es fricción. Mata tu flujo. Los ejecutores de tareas simples como `make` o `just` te dan atajos. **`axes` te da un lenguaje universal.**

`axes` no es solo otro ejecutor de tareas. Es el **lenguaje de comandos** que une a todo tu stack. Te permite componer, parametrizar y estandarizar flujos de trabajo para CUALQUIER herramienta, en CUALQUIER lenguaje. Tu `package.json` conoce `npm`, tu `Makefile` conoce `make`. **`axes` es el director de orquesta que los conoce a todos**, convirtiendo tu caótica colección de herramientas en una sinfonía.

### ¿Quién Dijo que Tenías que Elegir Entre Potencia y Velocidad?

Durante años, la elección ha sido entre:

- **Ejecutores Simples (`just`, `make`):** Muy rápidos, pero limitados. Son gestores de alias glorificados.
- **Orquestadores Complejos (`Bazel`, `Gradle`):** Increíblemente potentes, pero notoriamente lentos, complejos y rígidos.

**`axes` rompe este compromiso.** Ofrecemos las capacidades avanzadas de orquestación de sistemas complejos a una velocidad que rivaliza (y a menudo supera) a los ejecutores más simples.

| Herramienta | Ejecución de Script en Caliente | Características de Orquestación |
| :---------  | :-----------------------------: | :-----------------------------: |
| `just`      | **~38 ms**                      |            Básicas              |
| `task`      | ***~40 ms**                     |          **Advanced**           |
| **`axes`**  | **~35 ms**                      |          **Avanzadas**          |

*Benchmarks ejecutados en una máquina de desarrollo estándar ejecutando un script simple de "hola mundo", Observando únicamente el tiempo de inicio, resolución, ejecución y cierre, obteniendo el tiempo mínimo promedio de conjuntos de 200 ejecuciones.*

Esto lo conseguimos través de una arquitectura obsesionada con el rendimiento.

- **Compilación JIT a AST:** La primera vez que ejecutas un script, `axes` actúa como un compilador Just-in-Time. Parsea tu `axes.toml`, resuelve toda la herencia y composición, y lo compila en un **Árbol de Sintaxis Abstracta (AST)** altamente optimizado.
- **Caché Binario Persistente:** Este AST se guarda en un caché binario (`.axes/config.cache.bin`).
- **Ejecuciones en Caliente Instantáneas:** Cada ejecución posterior omite por completo el trabajo costoso. `axes` deserializa el AST pre-compilado del caché binario—una operación órdenes de magnitud más rápida que el parseo de texto—y lo ejecuta.

**El resultado: pagas el coste de la orquestación una sola vez. Obtienes la velocidad de un ejecutor simple cada vez después.**

- ⚙️ **[Referencia de Arquitectura completa (`TECNICAL.md`)](./TECNICAL.md):** Si te interesa investigar más a fondo la arquitectura de `axes`, el mejor lugar es viendo el código, pero este es el segundo mejor lugar.

---

### La Filosofía `axes`: Más que un Ejecutor de Tareas

`axes` se construye sobre una base que las herramientas simples ignoran.

- **Orquestación, no solo Ejecución:** `axes` entiende que los proyectos tienen relaciones. Organízalos en árboles (`app/api`, `app/web`). Los hijos heredan y sobrescriben configuraciones. Define una vez, usa en todas partes.
- **Ergonomía, no solo Atajos:** Tus scripts se convierten en aplicaciones de línea de comandos de primera clase.

    ```toml
    # Scripts como Funciones: Parametriza, valida y establece valores por defecto.
    [scripts]
    deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
    ```

    No más scripts `bash` frágiles para parsear argumentos.
- **Robustez por Diseño:** `axes` identifica los proyectos por un `UUID` inmutable, no por una ruta de archivo frágil. Renombra o mueve tus directorios libremente—`axes` nunca perderá el rastro de tus proyectos.

---

### Instalación (30 Segundos para un Mejor Flujo de Trabajo)

`axes` es un único binario sin dependencias.

1. Ve a la [**página de Releases de `axes` en GitHub**](https://github.com/RetypeOS/axes/releases).
2. Descarga el archivo para tu sistema operativo.
3. Descomprímelo y mueve el ejecutable `axes` a un directorio en tu `PATH`.
4. Abre una **nueva terminal** y verifica con `axes --version`.

---

### `axes` en Acción: Un Vistazo al Poder

#### 1. Comandos Universales y Conscientes del Contexto

Ejecuta un script en el directorio actual. La sintaxis es simple y predecible.

```sh
# Ejecuta el script 'build' definido en el axes.toml más cercano
axes build --release
```

#### 2. Flujos de Trabajo Multiplataforma sin Esfuerzo

Define un comando una vez. Funciona para todo tu equipo, en cualquier SO.

```toml
[scripts.browse]
desc = "Abre la documentación local en el navegador."
windows = "start http://localhost:8080"
macos   = "open http://localhost:8080"
linux   = "xdg-open http://localhost:8080"
```

#### 3. Composición Dinámica en Tiempo Real

Ejecuta comandos y usa su salida al instante.

```toml
[scripts]
# Etiqueta una imagen Docker con el hash corto de git actual
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Sesiones de Enfoque Inmersivo

Sumérgete en un sub-proyecto. `axes` configura y desmantela tu entorno por ti.

```toml
# en my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Se ejecuta al entrar
at_exit  = "docker-compose down"       # Se ejecuta al salir
```

```sh
$ axes my-app/api # Inicia una sesión. `at_start` se ejecuta automáticamente.

(axes: my-app/api) $ axes test  # No necesitas repetir el contexto.
(axes: my-app/api) $ exit       # `at_exit` se ejecuta automáticamente.
```

**Tu entorno de desarrollo, bajo demanda.**

---

### ¿Listo para Dirigir tu Propia Orquesta?

La fricción que sientes cada día no es un requisito. Es un problema con una solución. `axes` es esa solución.

- ➡️ **[Guía de Inicio Rápido (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Construye tu primer monorepo orquestado en 15 minutos.
- 📖 **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La referencia definitiva de cada característica.
- ⌨️ **[Referencia de Comandos (`COMMAND.md`)](./COMMAND.md):** Una guía completa de cada comando de la CLI.

### Únete a la Revolución del Flujo de Trabajo

`axes` es más que una herramienta; es un movimiento para devolver el control y la coherencia al desarrollo. Tu voz es crucial.

- **Encuentra un Bug o tienes una Idea Genial:** [**Abre un Issue**](https://github.com/RetypeOS/axes/issues)
- **Quieres Contribuir con Código:** ¡Los Pull Requests son siempre bienvenidos!

**Instala `axes` hoy. Deja de buscar comandos. Céntrate en lo que realmente importa: **darle vida a tu software**, y deja que `axes` se preocupe del cómo.**
