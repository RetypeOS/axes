# Hoja de Ruta del Proyecto `axes`

¡Bienvenido a la hoja de ruta de `axes`! Este documento describe la visión a corto y largo plazo para el proyecto. Es una guía para los desarrolladores principales y un punto de partida para los miembros de la comunidad que deseen contribuir.

## Cómo Contribuir

¡Tu ayuda es bienvenida! Si ves una tarea que te interese, especialmente las marcadas con `[contribución bienvenida]`, el proceso ideal es:

1. Asegúrate de que no haya un Pull Request abierto para esa tarea.
2. Abre un "Issue" en GitHub para discutir tu enfoque y que podamos asignártelo. Esto evita trabajo duplicado.
3. ¡Empieza a trabajar en tu Pull Request!

## Estado Actual: `v0.2.0-beta`

`axes` se encuentra en su primera fase **Beta**. Esto significa:

* **Núcleo Estable:** La arquitectura principal (dispatcher, handlers, interpolador, sistema de caché) es robusta y está bien probada.
* **API de `axes.toml` Definida:** La sintaxis del `axes.toml`, incluyendo `[scripts]`, herencia y el sistema `<axes::params::...>`, está completa en características para la v1.0.
* **Listo para Probar:** La herramienta está lista para ser usada en proyectos reales. Se esperan bugs, y el feedback de los usuarios es crucial en esta fase.

---

## Hoja de Ruta Inmediata (El Camino a la v1.0)

Estos son los hitos que nos llevarán a una versión 1.0 estable y pulida.

### Hito 1: La Experiencia de Usuario "Premium" (v0.3.0)

**Objetivo:** Hacer que la interacción diaria con `axes` sea lo más fluida, rápida e intuitiva posible.

* `[ ]` **Implementar Autocompletado de la Shell:** `[contribución bienvenida]`
  * **Descripción:** Integrar `clap_complete` para generar scripts de autocompletado para `bash`, `zsh`, `fish`, etc. Debe autocompletar dinámicamente contextos de proyecto, acciones y nombres de scripts.
  * **Valor:** La mejora de calidad de vida más importante para la usabilidad diaria.
* `[ ]` **Implementar la TUI de Bienvenida:**
  * **Descripción:** Al ejecutar `axes` sin argumentos, lanzar una TUI (Interfaz de Usuario de Terminal) de solo lectura que muestre el árbol de proyectos y permita explorar los scripts disponibles.
  * **Valor:** Transforma la primera impresión y facilita enormemente la "descubribilidad" en ecosistemas complejos.
* `[ ]` **Estandarizar y Embellecer la Salida:** `[contribución bienvenida]`
  * **Descripción:** Crear un módulo `ui/printer` y usar una crate como `cli-table` para estandarizar la salida de `info`, `alias list`, etc., en tablas bien formateadas.
  * **Valor:** Proporciona una identidad visual cohesiva y profesional a la herramienta.

### Hito 2: Control de Herencia y Lógica de Scripts Avanzada (v0.4.0)

**Objetivo:** Dar a los usuarios un control granular sobre la configuración heredada y desbloquear patrones de scripting avanzados.

* `[ ]` **Implementar Herencia Privada/Pública con `_`:**
  * **Descripción:** Modificar el `config_resolver` para que las claves en `[vars]`, `[env]`, y `[scripts]` que comiencen con un guion bajo (`_`) no sean heredadas por los proyectos hijos.
  * **Valor:** Permite la encapsulación y la definición de "helpers" internos en un proyecto padre sin contaminar el espacio de nombres de los hijos.
* `[ ]` **Implementar Comandos Multiplataforma en Secuencias:**
  * **Descripción:** Extender el parser del `axes.toml` para que dentro de una secuencia de `[scripts]`, se pueda definir un paso individual como una tabla multiplataforma.

        ```toml
        # Sintaxis a soportar
        deploy = [
            "<axes::scripts::build>",
            { windows = "win-deploy.ps1", linux = "./deploy.sh" },
            "echo 'Desplegado!'"
        ]
        ```

  * **Valor:** Desbloquea la capacidad de crear flujos de trabajo complejos que son, paso a paso, completamente multiplataforma.

### Hito 3: Estabilización y Ecosistema (v1.0.0)

**Objetivo:** Preparar `axes` para su lanzamiento oficial, enfocándose en la robustez y en facilitar su adopción.

* `[ ]` **Implementar `axes validate`:**
  * **Descripción:** Un comando que escanee todo el `index.bin` en busca de inconsistencias (rutas que ya no existen, enlaces de padre rotos) y ofrezca reportes o reparaciones interactivas.
  * **Valor:** Una herramienta de diagnóstico crucial para la confianza del usuario a largo plazo.
* `[ ]` **Soporte Nativo para Archivos `.env`:** `[contribución bienvenida]`
  * **Descripción:** Añadir una clave `[env].load = ".env"` al `axes.toml` que cargue automáticamente las variables de un archivo `.env` en el entorno de ejecución de los scripts.
  * **Valor:** Una integración muy solicitada que simplifica enormemente la gestión de secretos y configuraciones locales.
* `[ ]` **Congelación de la API y Documentación Final:**
  * **Descripción:** Realizar una revisión final de todas las APIs (CLI y `axes.toml`) y declararlas estables para la v1.0. Completar y pulir toda la documentación.
  * **Valor:** La garantía de estabilidad que los usuarios necesitan para adoptar `axes` en producción.

---

## Ideas a Largo Plazo (Post-v1.0 / El Futuro)

Estas son características más ambiciosas que se considerarán una vez que el núcleo del sistema sea estable.

* `[ ]` **Motor de Plantillas (`init` 2.0):** Transformar `init` en un motor de andamiaje completo que use plantillas de `~/.config/axes/templates/`.
* `[ ]` **Cambio de Sesión (`axes switch <contexto>`):** La capacidad de cambiar de una sesión de proyecto a otra sin necesidad de `exit` y volver a entrar.
* `[ ]` **Centralización de Cachés:** Mover todos los archivos de caché (`.axes/*.bin`) a un directorio centralizado (`~/.config/axes/cache/`) para mantener limpios los directorios de los proyectos.
* `[ ]` **Integración con Git:** Añadir tokens dinámicos como `<axes::git::branch>` o `<axes::git::commit_hash>`.

---

## ¡Ayúdanos a Probar! (Peticiones de Testing para la Beta)

¡La mejor forma de contribuir ahora mismo es probando `axes` en tus flujos de trabajo! Estamos especialmente interesados en feedback sobre las siguientes áreas:

1. **El Sistema de Parámetros:** Intenta crear scripts complejos usando `<axes::params::...>` en todas sus variantes. ¿Es intuitivo? ¿Encuentras algún caso borde que no funcione?
2. **La Composición de Scripts:** Crea scripts que se llamen unos a otros (`<axes::scripts::...>`) y que usen ejecución paralela (`>`). Intenta romper la detección de ciclos.
3. **Operaciones de Refactorización:** Usa `link`, `rename`, `unregister` y `delete` (con sus flags) en un monorepo de prueba. ¿Es el comportamiento siempre el esperado? ¿Son los mensajes claros?
4. **Cancelación con `Ctrl+C`:** Lanza un script de larga duración (ej. `[scripts] wait = "sleep 30"`) e intenta cancelarlo. ¿Responde la herramienta como esperas?

**¿Cómo reportar feedback?**
Por favor, abre un "Issue" en nuestro [repositorio de GitHub](https://github.com/RetypeOS/axes/issues). ¡Cualquier feedback es increíblemente valioso
