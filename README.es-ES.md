

# Lightfriend

Asistente de IA para teléfonos básicos, diseñado para que nadie más pueda ver tus chats o datos personales, incluso mientras la IA los procesa. Todo el código es de código abierto y las pruebas criptográficas permiten a cualquier persona verificar de forma independiente qué código se está ejecutando en producción.

Accede a WhatsApp, Telegram, Signal, correo electrónico, calendario, búsqueda web y más mediante SMS y llamadas de voz: no se requieren aplicaciones ni smartphone.

Stack completo en Rust (backend Axum, frontend Yew WebAssembly) con un servidor Matrix para mensajería multiplataforma.

## Arquitectura de privacidad verificable

El objetivo de privacidad moldea toda la arquitectura: aislamiento de hardware, almacenamiento cifrado, gestión independiente de claves, atestación remota e inferencia de IA verificable. Las secciones a continuación describen los controles implementados y lo que establecen las pruebas disponibles.

- **Aislamiento de hardware**: La aplicación de producción se ejecuta en un **AWS Nitro Enclave** sin inicio de sesión SSH, depurador interactivo, almacenamiento persistente ni red externa directa.
- **Atestación remota**: El hardware de AWS firma un documento de atestación que contiene la medición del enclave (PCR0/PCR1/PCR2).
- **Compilaciones reproducibles**: GitHub Actions compila la imagen del enclave y publica los valores PCR para que puedan compararse con la medición reportada por producción.
- **Registro público de código**: Las huellas digitales de las imágenes aprobadas se publican en un [contrato inteligente de Arbitrum](https://lightfriend.ai/trust-chain).
- **Gestión independiente de claves**: [Marlin KMS](https://github.com/marlinprotocol/oyster-monorepo) evalúa la atestación del enclave antes de liberar las claves de cifrado. El operador de Lightfriend no aprovisiona manualmente la clave maestra.
- **Inferencia de IA verificable**: [Tinfoil](https://tinfoil.sh) publica el código fuente y las pruebas de atestación para su entorno de inferencia de computación confidencial.

### Verifícalo tú mismo

```bash
./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc
```

Esto verifica la firma de atestación de AWS, compara los valores PCR reportados con la compilación pública y consulta la lista pública de aprobaciones. La atestación verifica la identidad del despliegue; no prueba que el software esté libre de errores.

### Llamadas de voz opcionales

Las llamadas de voz actualmente utilizan OpenAI Realtime para una experiencia más rápida y natural. El audio y las transcripciones de las llamadas se procesan fuera de la cadena de confianza verificable de forma independiente de Lightfriend. OpenAI indica que los datos de la API no se utilizan para el entrenamiento a menos que el cliente lo active, pero los registros predeterminados de monitoreo de abuso de Realtime pueden retener el contenido del cliente hasta por 30 días. Las llamadas de voz son opcionales y Lightfriend cambiará tan pronto como una alternativa de voz de código abierto y attestada adecuada pueda ofrecer una experiencia comparable.

- Attestación en vivo: `https://lightfriend.ai/.well-known/lightfriend/attestation`
- Explicación completa: [lightfriend.ai/trustless](https://lightfriend.ai/trustless)
- Panel de control de la cadena de confianza: [lightfriend.ai/trust-chain](https://lightfriend.ai/trust-chain)

## Desarrollo local

```bash
# Terminal 1: Backend
cd backend && cargo run

# Terminal 2: Frontend
cd frontend && trunk serve
```

- **API del backend**: http://localhost:3000
- **Frontend**: http://localhost:8080

## Docker (Enclave)

La imagen del enclave empaqueta todo (PostgreSQL, Tuwunel, puentes mautrix, backend de Lightfriend) en un solo contenedor supervisado por supervisord.

```bash
# Build for current platform (local testing)
just build-local

# Start
just up

# View logs
just logs
```

Consulta `just --list` para ver todos los comandos disponibles.

## Documentación

- [Guía de configuración de Matrix](docs/MATRIX_SETUP_GUIDE.md) - configuración manual de Matrix para desarrollo local
- [Configuración de infraestructura](docs/INFRASTRUCTURE_SETUP.md) - despliegue en la nube con Terraform
- [CLAUDE.md](CLAUDE.md) - arquitectura del proyecto y guía de desarrollo

## Licencia

Este proyecto está licenciado bajo la **Licencia Pública General Afirmativa de GNU v3 (AGPLv3)**. Consulta el archivo LICENSE para obtener más detalles.

El nombre "Lightfriend" y cualquier marca comercial asociada (incluidos logotipos, iconos o elementos visuales) son propiedad de Rasmus Ahtava. Estos elementos no se incluyen en la licencia AGPLv3 y no pueden utilizarse sin permiso, especialmente para fines comerciales o de manera que implique aprobación o afiliación. Las bifurcaciones o versiones derivadas deben utilizar un nombre y una marca diferentes para evitar confusiones.
