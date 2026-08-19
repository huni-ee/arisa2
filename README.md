# Arisa

카카오톡 안드로이드 앱의 데이터베이스와 연동하여 실시간 채팅 봇을 구현할 수 있는 gRPC 기반 프레임워크입니다.

메시지와 채팅방 이벤트를 실시간으로 수신하고, 메시지 전송, 미디어 전송, 읽음 처리, 사용자 및 채팅방 조회 등의 기능을 제공합니다.

## 주요 기능

- 카카오톡 메시지 및 피드 이벤트 실시간 수신
- 텍스트 메시지 및 미디어 전송
- 메시지 읽음 처리 및 채팅방 입장
- 사용자, 채팅방 및 메시지 정보 조회
- 카카오톡 데이터베이스 SQL 조회
- 암호화된 데이터 복호화
- Python 비동기 클라이언트 제공
- 일반 사용자, 업무 프로필, 듀얼 앱 및 보안 폴더 지원
- 최대 300 MiB gRPC 메시지 송수신 지원
- ARM 및 x86 계열 안드로이드 기기 지원

## 필요 조건

- 카카오톡이 설치된 안드로이드 기기
- 안드로이드 루트 권한
- ADB(Android Debug Bridge)
- Linux, macOS 또는 Windows PC
- Python 클라이언트 사용 시 Python 3.11 이상

지원되는 안드로이드 아키텍처는 다음과 같습니다.

- `armeabi-v7a`
- `arm64-v8a`
- `x86`
- `x86_64`

## 설치

`arisa_control` 스크립트는 연결된 기기의 아키텍처를 자동으로 확인하고, 알맞은 Arisa 바이너리와 `fileprovider.apk`를 다운로드하여 설치합니다.

### Linux/macOS

```bash
wget https://github.com/huni-ee/arisa2/releases/latest/download/arisa_control
chmod +x arisa_control
./arisa_control install
```

### Windows PowerShell

```powershell
wget https://github.com/huni-ee/arisa2/releases/latest/download/arisa_control.ps1 -OutFile arisa_control.ps1
.\arisa_control.ps1 install
```

설치가 끝나면 Arisa를 실행합니다.

### 실행

#### Linux/macOS

```bash
./arisa_control start
```

#### Windows PowerShell

```powershell
.\arisa_control.ps1 start
```

기본적으로 gRPC 서버는 다음 주소에서 실행됩니다.

```text
0.0.0.0:3000
```

## 제어 명령어

Linux/macOS에서는 다음 형식으로 사용합니다.

```bash
./arisa_control <명령어>
```

Windows에서는 다음 형식으로 사용합니다.

```powershell
.\arisa_control.ps1 <명령어>
```

| 명령어 | 설명 |
| --- | --- |
| `install` | Arisa 바이너리와 FileProvider를 설치합니다. |
| `start` | Arisa 서비스를 시작합니다. |
| `stop` | Arisa 서비스를 종료합니다. |
| `status` | Arisa 실행 상태를 확인합니다. |
| `config` | Arisa 설정을 변경합니다. |
| `remove` | Arisa 바이너리를 제거합니다. |
| `all_remove` | Arisa, 설정 파일 및 FileProvider를 모두 제거합니다. |
| `install_redroid` | ReDroid를 설치합니다. (Linux 전용) |

## 설정

다음 명령어를 실행하여 Arisa 환경 변수를 설정할 수 있습니다.

```bash
./arisa_control config
```

Windows:

```powershell
.\arisa_control.ps1 config
```

설정은 안드로이드 기기의 다음 경로에 저장됩니다.

```text
/data/local/tmp/arisa_config.json
```

| 환경 변수 | 설명 | 기본값 |
| --- | --- | --- |
| `ARISA_BIND` (선택) | 서버 바인딩 주소를 변경할 때 사용합니다. | `0.0.0.0:3000` |
| `ARISA_UID` (선택) | 듀얼 메신저, 업무 프로필 또는 보안 폴더를 사용할 때 Android 사용자 ID를 지정합니다. (예: `10`) | `0` |
| `ARISA_CALLING_PKG` (선택) | 비루트 환경에서 사용할 Android 호출 패키지를 지정합니다. (예: `com.termux`) | `com.android.shell` |
| `ARISA_DB_PULL_DELAY` (선택) | 데이터베이스 폴링 간격을 밀리초 단위로 지정합니다. | `100` |

## Python

```bash
uv add "git+https://github.com/huni-ee/arisa2.git#subdirectory=python" --branch main
```

## gRPC API

Arisa는 다음 기능을 gRPC API로 제공합니다.

### 이벤트

- `SubscribeEvents`: 메시지와 채팅방 피드 이벤트 실시간 구독

### 메시지 및 채팅방 작업

- `Reply`: 텍스트 메시지 전송
- `SendMedia`: 단일 또는 여러 미디어 파일 전송
- `MarkRead`: 채팅방 메시지 읽음 처리
- `EnterChannel`: 채팅방 입장

### 데이터 조회

- `GetUser`: 사용자 조회
- `GetUsers`: 여러 사용자 조회
- `GetChannel`: 채팅방 조회
- `GetMessage`: 메시지 조회
- `GetMessages`: 여러 메시지 조회
- `GetChannelMembers`: 채팅방 참여자 조회
- `RawQuery`: 카카오톡 데이터베이스 SQL 조회
- `Decrypt`: 암호화 데이터 복호화
- `GetCredential`: 인증 정보 조회

전체 API 정의는 [`proto/arisa.proto`](proto/arisa.proto)를 참고하세요.

## 원본 프로젝트

이 저장소는 다음 프로젝트를 포크하여 수정한 버전입니다.

- 원본 저장소: [ye-seola/arisa2](https://github.com/ye-seola/arisa2)
- 현재 저장소: [huni-ee/arisa2](https://github.com/huni-ee/arisa2)
- `fileprovider.apk` 출처: [ye-seola/old-arisa-deploy](https://github.com/ye-seola/old-arisa-deploy)

원본 프로젝트를 개발한 [zugu님](https://github.com/ye-seola)께 감사드립니다.

## 면책 조항

본 프로젝트의 사용 또는 남용으로 발생하는 계정 제한, 데이터 손실, 기기 손상 및 기타 모든 불이익에 대해 개발자는 책임지지 않으며, 그 책임은 사용자 본인에게 있습니다. 사용자는 관련 법률과 서비스 이용약관을 준수할 책임이 있습니다.

## 라이선스

이 프로젝트는 [Apache License 2.0](LICENSE)에 따라 배포됩니다.
