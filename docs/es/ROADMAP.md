<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../ROADMAP.md">English</a> •
  <a href="./ROADMAP.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../README.md), que es la fuente canónica de la documentación.

# Hoja de Ruta del Proyecto `axes`

¡Bienvenido a la hoja de ruta de `axes`! Este documento describe nuestra ambiciosa visión para transformar `axes` de un orquestador de clase mundial en el sistema de construcción inteligente y definitivo para el desarrollo moderno. Sirve como una guía para nuestra misión principal y un llamado a la acción para los colaboradores de la comunidad.

## Cómo Contribuir

Tu experiencia es invaluable. Si ves una característica que te entusiasma, especialmente aquellas marcadas con `[contribución bienvenida]`, el proceso ideal es:

1. Revisa los Issues o Pull Requests existentes para evitar duplicar trabajo.
2. Abre un nuevo Issue para discutir tu estrategia de implementación. Esto nos permite alinearnos en la arquitectura y asignarte la tarea.
3. ¡Construyamos juntos el futuro de `axes`!

## Estado Actual: `v0.3.0-beta` — Hito de Arquitectura "Juggernaut"

Con la versión `v0.3.0`, hemos reestructurado con éxito el núcleo de `axes`. Esta versión "Juggernaut" establece una nueva base de rendimiento, robustez y consistencia multiplataforma.

* **AST Universal y Caché Portable:** El motor de compilación ha sido reescrito. `axes` ahora genera un **Árbol de Sintaxis Abstracta (AST) agnóstico a la plataforma**, lo que significa que los archivos de caché binaria son **100% portables** entre Windows, macOS y Linux. Esta es una característica revolucionaria para equipos multiplataforma.
* **Optimización Just-In-Time (JIT):** Se introdujo un paso final de "especialización" en memoria antes de la ejecución. Esto nos da la flexibilidad de una caché universal con la velocidad pura e intransigente de un ejecutor específico de la plataforma. Los benchmarks confirman que `axes` es ahora significativamente más rápido que sus predecesores y competidores en escenarios realistas de alta carga.
* **Sintaxis Mejorada y Robustez:** La sintaxis de `axes.toml` para scripts y variables es ahora más potente, ergonómica y estrictamente validada para prevenir errores del usuario.
* **Ejecución Efímera (`_`):** Ahora es posible ejecutar scripts en proyectos no registrados, una característica potente para CI/CD y flujos de trabajo temporales.

---

## El Camino a `v1.0` — De Orquestador a Sistema de Construcción Inteligente

Nuestro camino hacia `v1.0` se centra en construir sobre la nueva arquitectura "Juggernaut". Estabilizaremos, mejoraremos la experiencia del usuario y luego entregaremos la característica fundamental de un sistema de construcción inteligente: el caché de artefactos.

### **Hito 1: El Hito de "Pulido y Estabilidad" (`v0.4.0`)**

**Objetivo:** Consolidar la arquitectura `v0.3.0`, refinar la experiencia del usuario y asegurar una estabilidad a prueba de fallos. Esta es nuestra prioridad inmediata.

* `[ ]` **Revisión Arquitectónica Final y Correcciones Menores:**
  * **Descripción:** Llevar a cabo una revisión exhaustiva de todos los módulos y manejadores del núcleo, aplicando optimizaciones finales, mejorando la documentación y corrigiendo cualquier error menor descubierto después de la refactorización de v0.3.0.
  * **Valor:** Asegura que la nueva base sea impecable antes de construir sobre ella nuevas características importantes.
* `[ ]` **Implementar Autocompletado de Shell:** `[contribución bienvenida]`
  * **Descripción:** Proporcionar autocompletado dinámico y sensible al contexto para `bash`, `zsh` y `fish`. El motor debe sugerir inteligentemente contextos de proyecto, alias y scripts disponibles para un contexto dado.
  * **Valor:** La mejora de calidad de vida más impactante para la descubribilidad y usabilidad diaria.
* `[ ]` **Implementar la TUI "Orquestadora" (MVP):**
  * **Descripción:** Al ejecutar `axes` sin argumentos, lanzar una Interfaz de Usuario de Terminal (TUI) básica e interactiva. Esta TUI visualizará el árbol de proyectos y permitirá a los usuarios navegar y seleccionar scripts disponibles para el contexto actual.
  * **Valor:** Transforma la primera impresión de `axes` en una experiencia guiada y premium, haciendo que los monorepos complejos sean instantáneamente navegables.

### **Hito 2: El Hito de "Construcción Inteligente" (`v0.5.0`)**

**Objetivo:** Transformar `axes` de un *ejecutor* de tareas en un sistema de construcción eficiente que evita el trabajo redundante.

* `[ ]` **Implementar Caché de Artefactos (MVP):**
  * **Descripción:** Introducir un mecanismo de caché de tareas basado en sumas de verificación de archivos de entrada. Una nueva sección `[scripts.mi_script.cache]` en `axes.toml` permitirá a los usuarios declarar `sources` (archivos/patrones de archivos de entrada). `axes` calculará una suma de verificación de estado y omitirá la ejecución del script si ninguna fuente ha cambiado desde la última ejecución exitosa.
  * **Valor:** Esta es la característica fundamental para la productividad del desarrollador, ahorrando una inmensa cantidad de tiempo en flujos de trabajo diarios y pipelines de CI/CD al evitar costosas re-compilaciones, re-pruebas y re-empaquetados.

### **Hito 3: El Hito de "Ecosistema y Estabilidad" (`v1.0.0`)**

**Objetivo:** Preparar `axes` para su lanzamiento oficial y listo para producción con características que fomentan la confianza a largo plazo y simplifican la integración.

* `[ ]` **Implementar comando `axes doctor`:**
  * **Descripción:** Un comando de verificación de salud exhaustivo que encuentra y ofrece corregir inconsistencias como enlaces padre rotos en el índice, proyectos cuyas rutas ya no existen y archivos de caché huérfanos. Esto evoluciona el comando `repair` existente.
  * **Valor:** Una herramienta de diagnóstico y reparación crucial que fomenta la confianza del usuario.
* `[ ]` **Soporte Nativo para Archivos `.env`:** `[contribución bienvenida]`
  * **Descripción:** Agregar una clave como `[env].load = ".env"` para descubrir y cargar automáticamente variables desde un archivo `.env` especificado en el entorno de ejecución del script.
  * **Valor:** Una característica altamente solicitada que se alinea con las prácticas modernas de desarrollo.
* `[ ]` **Congelación de la API Final y Revisión de la Documentación:**
  * **Descripción:** Realizar una revisión final de la sintaxis de `axes.toml` y los contratos de comandos CLI. Declararlos estables para `v1.0`. Asegurar que toda la documentación esté completa, pulida y llena de ejemplos del mundo real.
  * **Valor:** La promesa de estabilidad esencial para la adopción en entornos de producción.

---

## **El Futuro (Post-`v1.0`)**

Estas son características ambiciosas y revolucionarias que exploraremos una vez que la base de `v1.0` sea sólida.

* `[ ]` **El Demonio `axes` (Daemon):** Un proceso en segundo plano de larga ejecución para un caché de construcción casi instantáneo y reactividad.
* `[ ]` **Caché Remoto:** Compartir el caché de artefactos a través de un equipo o una granja de CI.
* `[ ]` **Motor de Plantillas (`init 2.0`):** Un potente motor de andamiaje para generar nuevos proyectos a partir de plantillas.
* `[ ]` **Características de Scripting Avanzado:** Scopes privados (prefijo `_` para elementos no heredables), claves de caché avanzadas y tokens dinámicos (`<git::branch>`).

---

## **Llamada a Probadores (Fase Beta)**

La contribución más valiosa en este momento es **usar `axes v0.3.0` y darnos feedback**. Estamos particularmente interesados en:

1. **Caché Multiplataforma:** Si trabajas en un equipo con múltiples sistemas operativos, intenta subir tu directorio `.axes-cache` al repositorio. ¿Funciona sin problemas para tus colegas?
2. **El Contexto `_`:** Lleva el modo de ejecución efímera al límite en tus pipelines de CI o pruebas locales.
3. **Monorepos del Mundo Real:** Integra `axes` en uno de tus proyectos complejos existentes. ¿Qué desafíos enfrentaste? ¿Qué características faltaron?

**¿Cómo reportar feedback?**
Por favor, abre un Issue en nuestro [repositorio de GitHub](https://github.com/retypeos/axes/issues). Cada pieza de feedback es un paso hacia la construcción de una herramienta mejor.
