<#
.SYNOPSIS
Manages the Arisa service on an Android device via ADB.
.DESCRIPTION
Selects an ADB device, detects its ABI, and installs or controls the matching Arisa binary.
#>
param(
    [Parameter(Position = 0, Mandatory = $false)]
    [ValidateSet('status', 'start', 'stop', 'install', 'config', 'remove', 'allremove')]
    [string]$Action = 'status'
)

$script:ReleaseBaseUrl = 'https://github.com/huni-ee/arisa2/releases/latest/download'
$script:ArisaConfigPath = '/data/local/tmp/arisa_config.json'
$script:FileProviderApkUrl = "$($script:ReleaseBaseUrl)/fileprovider.apk"
$script:FileProviderPackage = 'io.zugu.fileprovider'
$script:DefaultArisaBind = '0.0.0.0:3000'
$script:DefaultArisaUid = 0
$script:DefaultArisaCallingPackage = 'com.android.shell'
$script:DefaultArisaDbPullDelay = 100
$script:AdbSerial = $null
$script:DeviceArchitecture = $null
$script:ArisaAssetName = $null
$script:ArisaProcessName = $null
$script:ArisaBinaryUrl = $null
$script:ArisaBinaryPath = $null
$script:ConfigArisaBind = $null
$script:ConfigArisaUid = $null
$script:ConfigArisaCallingPackage = $null
$script:ConfigArisaDbPullDelay = $null
$script:ArisaEnvironmentPrefix = $null

function Test-AdbInstalled {
    if (-not (Get-Command adb -ErrorAction SilentlyContinue)) {
        Write-Host 'adb is not installed. Please install Android SDK Platform-Tools and add adb to PATH.'
        return $false
    }
    return $true
}

function Invoke-SelectedAdb {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$AdbArguments
    )

    & adb -s $script:AdbSerial @AdbArguments
}

function Invoke-RootAdbShell {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Command
    )

    $escapedCommand = $Command.Replace("'", "'\''")
    Invoke-SelectedAdb -AdbArguments @('shell', "su root sh -c '$escapedCommand'")
}

function Test-SuRootAccess {
    $uidOutput = @(Invoke-RootAdbShell -Command 'id -u' 2>$null)
    $remoteUid = ($uidOutput -join '').Trim()
    if ($remoteUid -ne '0') {
        Write-Host "Root access through 'su root' is required to run Arisa."
        return $false
    }

    return $true
}

function Select-AdbDevice {
    Write-Host 'Searching for connected devices...'
    $devices = @()

    foreach ($line in (& adb devices)) {
        if ($line -match '^(\S+)\s+device$') {
            $devices += $Matches[1]
        }
    }

    if ($devices.Count -eq 0) {
        Write-Host "Error: No adb devices found in 'device' state."
        Write-Host "Please connect a device via USB or use 'adb connect <ip>:<port>'."
        return $false
    }

    if ($devices.Count -eq 1) {
        $script:AdbSerial = $devices[0]
        Write-Host "Found 1 device. Using: $($script:AdbSerial)"
        return $true
    }

    Write-Host 'Multiple devices found. Please choose one to use:'
    for ($index = 0; $index -lt $devices.Count; $index++) {
        Write-Host "  $($index + 1)) $($devices[$index])"
    }

    $choice = Read-Host "Enter number (1-$($devices.Count))"
    $selectedNumber = 0
    if (-not [int]::TryParse($choice, [ref]$selectedNumber) -or
        $selectedNumber -lt 1 -or
        $selectedNumber -gt $devices.Count) {
        Write-Host 'Error: Invalid selection.'
        return $false
    }

    $script:AdbSerial = $devices[$selectedNumber - 1]
    Write-Host "Using device: $($script:AdbSerial)"
    return $true
}

function Set-ArisaConfigurationForDevice {
    $architectureOutput = @(Invoke-RootAdbShell -Command 'getprop ro.product.cpu.abi' 2>$null)
    $script:DeviceArchitecture = ($architectureOutput -join '').Trim()

    if (-not $script:DeviceArchitecture) {
        Write-Host 'Error: Failed to detect the selected device architecture.'
        return $false
    }

    Write-Host "Current device architecture: $($script:DeviceArchitecture)"

    switch ($script:DeviceArchitecture) {
        'armeabi-v7a' {
            $script:ArisaAssetName = 'arisa-armeabi-v7a'
        }
        'arm64-v8a' {
            $script:ArisaAssetName = 'arisa-arm64-v8a'
        }
        'x86' {
            $script:ArisaAssetName = 'arisa-x86'
        }
        'x86_64' {
            $script:ArisaAssetName = 'arisa-x86_64'
        }
        default {
            Write-Host "No Arisa binary is available for device architecture: $($script:DeviceArchitecture)"
            Write-Host 'Available architectures: armeabi-v7a, arm64-v8a, x86, x86_64'
            return $false
        }
    }

    $script:ArisaProcessName = $script:ArisaAssetName
    $script:ArisaBinaryUrl = "$($script:ReleaseBaseUrl)/$($script:ArisaAssetName)"
    $script:ArisaBinaryPath = "/data/local/tmp/$($script:ArisaAssetName)"
    return $true
}

function Get-ArisaPid {
    $pidOutput = @(Invoke-RootAdbShell -Command "pidof '$($script:ArisaProcessName)'" 2>$null)
    return ($pidOutput -join ' ').Trim()
}

function Test-ArisaBind {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    if ($Value -notmatch '^([0-9]{1,3}\.){3}[0-9]{1,3}:([0-9]{1,5})$') {
        return $false
    }

    $separatorIndex = $Value.LastIndexOf(':')
    $address = $Value.Substring(0, $separatorIndex)
    $portText = $Value.Substring($separatorIndex + 1)
    $port = 0
    if (-not [int]::TryParse($portText, [ref]$port) -or $port -lt 1 -or $port -gt 65535) {
        return $false
    }

    foreach ($part in $address.Split('.')) {
        $octet = 0
        if (-not [int]::TryParse($part, [ref]$octet) -or $octet -lt 0 -or $octet -gt 255) {
            return $false
        }
    }

    return $true
}

function Test-ArisaCallingPackage {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    return $Value -match '^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z][A-Za-z0-9_]*)+$'
}

function Set-DefaultArisaConfigValues {
    $script:ConfigArisaBind = $script:DefaultArisaBind
    $script:ConfigArisaUid = $script:DefaultArisaUid
    $script:ConfigArisaCallingPackage = $script:DefaultArisaCallingPackage
    $script:ConfigArisaDbPullDelay = $script:DefaultArisaDbPullDelay
}

function Write-ArisaConfig {
    $temporaryFile = [System.IO.Path]::GetTempFileName()

    try {
        $configObject = [ordered]@{
            ARISA_BIND = [string]$script:ConfigArisaBind
            ARISA_UID = [int]$script:ConfigArisaUid
            ARISA_CALLING_PKG = [string]$script:ConfigArisaCallingPackage
            ARISA_DB_PULL_DELAY = [long]$script:ConfigArisaDbPullDelay
        }
        $json = $configObject | ConvertTo-Json -Compress
        $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
        [System.IO.File]::WriteAllText($temporaryFile, $json, $utf8WithoutBom)

        Invoke-SelectedAdb -AdbArguments @('push', $temporaryFile, $script:ArisaConfigPath) | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to write Arisa configuration: $($script:ArisaConfigPath)"
            return $false
        }

        Invoke-RootAdbShell -Command "chmod 644 '$($script:ArisaConfigPath)'" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to set permissions on Arisa configuration: $($script:ArisaConfigPath)"
            return $false
        }

        return $true
    } finally {
        Remove-Item -LiteralPath $temporaryFile -Force -ErrorAction SilentlyContinue
    }
}

function Test-ArisaConfigExists {
    Invoke-RootAdbShell -Command "test -f '$($script:ArisaConfigPath)'" | Out-Null
    return $LASTEXITCODE -eq 0
}

function Ensure-ArisaConfig {
    if (Test-ArisaConfigExists) {
        return $true
    }

    Write-Host "Creating default Arisa configuration: $($script:ArisaConfigPath)"
    Set-DefaultArisaConfigValues
    if (-not (Write-ArisaConfig)) {
        return $false
    }

    Write-Host 'Arisa configuration created successfully.'
    return $true
}

function Read-ArisaConfig {
    $configOutput = @(Invoke-RootAdbShell -Command "cat '$($script:ArisaConfigPath)'" 2>$null)
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to read Arisa configuration: $($script:ArisaConfigPath)"
        return $false
    }

    $json = ($configOutput -join "`n").Trim()
    try {
        $config = $json | ConvertFrom-Json -ErrorAction Stop
    } catch {
        Write-Host "Failed to parse Arisa configuration: $($script:ArisaConfigPath)"
        return $false
    }

    $bind = [string]$config.ARISA_BIND
    if (-not (Test-ArisaBind -Value $bind)) {
        $bind = $script:DefaultArisaBind
    }

    $uid = 0
    if (-not [int]::TryParse([string]$config.ARISA_UID, [ref]$uid) -or $uid -lt 0) {
        $uid = $script:DefaultArisaUid
    }

    $callingPackage = [string]$config.ARISA_CALLING_PKG
    if (-not (Test-ArisaCallingPackage -Value $callingPackage)) {
        $callingPackage = $script:DefaultArisaCallingPackage
    }

    $dbPullDelay = 0L
    if (-not [long]::TryParse([string]$config.ARISA_DB_PULL_DELAY, [ref]$dbPullDelay)) {
        $dbPullDelay = $script:DefaultArisaDbPullDelay
    } elseif ($dbPullDelay -le 0) {
        $dbPullDelay = 1000
    }

    $script:ConfigArisaBind = $bind
    $script:ConfigArisaUid = $uid
    $script:ConfigArisaCallingPackage = $callingPackage
    $script:ConfigArisaDbPullDelay = $dbPullDelay
    return $true
}

function Set-ArisaEnvironmentPrefix {
    if (-not (Ensure-ArisaConfig)) {
        return $false
    }
    if (-not (Read-ArisaConfig)) {
        return $false
    }

    $script:ArisaEnvironmentPrefix = (
        "ARISA_BIND=$($script:ConfigArisaBind) " +
        "ARISA_UID=$($script:ConfigArisaUid) " +
        "ARISA_CALLING_PKG=$($script:ConfigArisaCallingPackage) " +
        "ARISA_DB_PULL_DELAY=$($script:ConfigArisaDbPullDelay)"
    )
    return $true
}

function Read-ValueWithExample {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Prompt,

        [Parameter(Mandatory = $true)]
        [string]$Example
    )

    $value = Read-Host "$Prompt[$Example]"
    if ([string]::IsNullOrEmpty($value)) {
        return $Example
    }
    return $value
}

function Confirm-ArisaConfigChange {
    while ($true) {
        $answer = Read-Host 'Are you sure you want to change this value? (y/n)'
        if ($answer -cmatch '^[yY]$') {
            return $true
        }
        if ($answer -cmatch '^[nN]$') {
            Write-Host 'Change cancelled.'
            return $false
        }
        Write-Host 'Please enter y or n.'
    }
}

function Save-ArisaConfig {
    if (-not (Write-ArisaConfig)) {
        return $false
    }

    Write-Host 'Arisa configuration saved successfully.'
    return $true
}

function Set-ArisaBindConfig {
    $newValue = Read-ValueWithExample -Prompt 'Enter ARISA_BIND: ' -Example $script:ConfigArisaBind
    if (-not (Test-ArisaBind -Value $newValue)) {
        Write-Host "Invalid IP:PORT. Using default: $($script:DefaultArisaBind)"
        return $true
    }

    Write-Host "ARISA_BIND: $($script:ConfigArisaBind) -> $newValue"
    if (-not (Confirm-ArisaConfigChange)) {
        return $true
    }

    $script:ConfigArisaBind = $newValue
    return (Save-ArisaConfig)
}

function Set-ArisaUidConfig {
    $userOutput = @(Invoke-RootAdbShell -Command 'pm list users' 2>$null)
    $users = @()

    foreach ($line in $userOutput) {
        if (([string]$line) -notmatch 'UserInfo\{([0-9]+):([^:]+):') {
            continue
        }

        $userId = [int]$Matches[1]
        $userName = $Matches[2]
        if ($userId -eq 0) {
            $users += [pscustomobject]@{
                Id = 0
                Name = 'Owner'
            }
            continue
        }

        $displayName = switch -CaseSensitive ($userName) {
            { $_ -in @('Work profile', '업무 프로필') } { 'Work profile'; break }
            { $_ -in @('DUAL_APP', '듀얼 앱') } { 'DUAL_APP'; break }
            { $_ -in @('Secure Folder', '보안 폴더') } { 'Secure Folder'; break }
            default { $null }
        }

        if ($null -ne $displayName) {
            $users += [pscustomobject]@{
                Id = $userId
                Name = $displayName
            }
        }
    }

    if ($users.Count -eq 0) {
        Write-Host 'No supported Android users were found.'
        return $false
    }

    Write-Host 'Select an Android user:'
    for ($index = 0; $index -lt $users.Count; $index++) {
        Write-Host "  $($index + 1)) $($users[$index].Name) (ID: $($users[$index].Id))"
    }

    $choice = Read-Host "Enter number (1-$($users.Count))"
    $selectedNumber = 0
    if (-not [int]::TryParse($choice, [ref]$selectedNumber) -or
        $selectedNumber -lt 1 -or
        $selectedNumber -gt $users.Count) {
        Write-Host 'Error: Invalid selection.'
        return $false
    }

    $selectedUser = $users[$selectedNumber - 1]
    Write-Host "ARISA_UID: $($script:ConfigArisaUid) -> $($selectedUser.Id) ($($selectedUser.Name))"
    if (-not (Confirm-ArisaConfigChange)) {
        return $true
    }

    $script:ConfigArisaUid = [int]$selectedUser.Id
    return (Save-ArisaConfig)
}

function Set-ArisaCallingPackageConfig {
    $newValue = Read-ValueWithExample `
        -Prompt 'Enter ARISA_CALLING_PKG: ' `
        -Example $script:ConfigArisaCallingPackage
    if (-not (Test-ArisaCallingPackage -Value $newValue)) {
        Write-Host "Invalid Android package name. Using default: $($script:DefaultArisaCallingPackage)"
        $newValue = $script:DefaultArisaCallingPackage
    }

    Write-Host "ARISA_CALLING_PKG: $($script:ConfigArisaCallingPackage) -> $newValue"
    if (-not (Confirm-ArisaConfigChange)) {
        return $true
    }

    $script:ConfigArisaCallingPackage = $newValue
    return (Save-ArisaConfig)
}

function Set-ArisaDbPullDelayConfig {
    $newValue = Read-ValueWithExample `
        -Prompt 'Enter ARISA_DB_PULL_DELAY (ms): ' `
        -Example ([string]$script:ConfigArisaDbPullDelay)
    $delay = 0L
    if (-not [long]::TryParse($newValue, [ref]$delay)) {
        Write-Host 'Invalid delay. Using 1000 ms.'
        $delay = 1000
    } elseif ($delay -le 0) {
        Write-Host 'The delay must be positive. Using 1000 ms.'
        $delay = 1000
    }

    Write-Host "ARISA_DB_PULL_DELAY: $($script:ConfigArisaDbPullDelay) -> $delay"
    if (-not (Confirm-ArisaConfigChange)) {
        return $true
    }

    $script:ConfigArisaDbPullDelay = $delay
    return (Save-ArisaConfig)
}

function Set-ArisaConfig {
    if (-not (Ensure-ArisaConfig)) {
        return $false
    }
    if (-not (Read-ArisaConfig)) {
        return $false
    }

    Write-Host 'Arisa configuration:'
    Write-Host "  1) ARISA_BIND ($($script:ConfigArisaBind))"
    Write-Host "  2) ARISA_UID ($($script:ConfigArisaUid))"
    Write-Host "  3) ARISA_CALLING_PKG ($($script:ConfigArisaCallingPackage))"
    Write-Host "  4) ARISA_DB_PULL_DELAY ($($script:ConfigArisaDbPullDelay) ms)"
    $choice = Read-Host 'Enter number (1-4)'

    switch ($choice) {
        '1' { return (Set-ArisaBindConfig) }
        '2' { return (Set-ArisaUidConfig) }
        '3' { return (Set-ArisaCallingPackageConfig) }
        '4' { return (Set-ArisaDbPullDelayConfig) }
        default {
            Write-Host 'Error: Invalid selection.'
            return $false
        }
    }
}

function Show-ArisaStatus {
    $arisaPid = Get-ArisaPid
    if ($arisaPid) {
        Write-Host "Arisa is working. PID: $arisaPid"
    } else {
        Write-Host 'Arisa is not running.'
    }
}

function Start-Arisa {
    $arisaPid = Get-ArisaPid
    if ($arisaPid) {
        Write-Host "Arisa is already running. PID: $arisaPid"
        return $true
    }

    Invoke-RootAdbShell -Command "test -x '$($script:ArisaBinaryPath)'" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Arisa is not installed at $($script:ArisaBinaryPath)."
        Write-Host "Run '$PSCommandPath install' first."
        return $false
    }

    if (-not (Set-ArisaEnvironmentPrefix)) {
        return $false
    }

    Write-Host 'Starting Arisa service in the background...'
    $startCommand = (
        "nohup sh -c 'while true; do sleep 1; echo; done | " +
        "$($script:ArisaEnvironmentPrefix) $($script:ArisaBinaryPath)' > /dev/null 2>&1 &"
    )
    Invoke-RootAdbShell -Command $startCommand | Out-Null
    Start-Sleep -Seconds 2

    $arisaPid = Get-ArisaPid
    if ($arisaPid) {
        Write-Host "Arisa service started. PID: $arisaPid"
        return $true
    }

    Write-Host 'Failed to start Arisa service.'
    return $false
}

function Stop-Arisa {
    $arisaPid = Get-ArisaPid
    if (-not $arisaPid) {
        Write-Host 'Arisa is not running.'
        return $true
    }

    Write-Host 'Stopping Arisa service...'
    Invoke-RootAdbShell -Command "kill -s SIGKILL $arisaPid" | Out-Null
    Start-Sleep -Seconds 1

    $stoppedPid = Get-ArisaPid
    if (-not $stoppedPid) {
        Write-Host 'Arisa service stopped.'
        return $true
    }

    Write-Host "Failed to stop Arisa service. PID $stoppedPid is still running."
    return $false
}

function Get-FileProviderPackagePath {
    $packageOutput = @(
        Invoke-RootAdbShell -Command "pm path '$($script:FileProviderPackage)'" 2>$null
    )
    return ($packageOutput -join '').Trim()
}

function Install-FileProvider {
    $temporaryApk = [System.IO.Path]::Combine(
        [System.IO.Path]::GetTempPath(),
        "fileprovider-$([guid]::NewGuid().ToString('N')).apk"
    )

    try {
        Write-Host "Downloading fileprovider.apk: $($script:FileProviderApkUrl)"
        try {
            Invoke-WebRequest `
                -Uri $script:FileProviderApkUrl `
                -OutFile $temporaryApk `
                -UseBasicParsing `
                -ErrorAction Stop
        } catch {
            Write-Host 'Failed to download fileprovider.apk.'
            return $false
        }

        Write-Host 'Installing fileprovider.apk on the selected device...'
        Invoke-SelectedAdb -AdbArguments @('install', '-r', $temporaryApk) | Out-Host
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'Failed to install fileprovider.apk.'
            return $false
        }

        $packagePath = Get-FileProviderPackagePath
        if (-not $packagePath.StartsWith('package:')) {
            Write-Host "Installation verification failed for $($script:FileProviderPackage)."
            return $false
        }

        Write-Host "Granting MANAGE_EXTERNAL_STORAGE to $($script:FileProviderPackage)..."
        Invoke-RootAdbShell `
            -Command "appops set $($script:FileProviderPackage) MANAGE_EXTERNAL_STORAGE allow" |
            Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to grant MANAGE_EXTERNAL_STORAGE to $($script:FileProviderPackage)."
            return $false
        }

        $appOpOutput = @(
            Invoke-RootAdbShell `
                -Command "appops get $($script:FileProviderPackage) MANAGE_EXTERNAL_STORAGE" `
                2>$null
        )
        $appOpMode = ($appOpOutput -join "`n").Trim()
        if ($appOpMode -notmatch 'allow') {
            Write-Host "MANAGE_EXTERNAL_STORAGE verification failed for $($script:FileProviderPackage)."
            return $false
        }

        Write-Host 'fileprovider.apk installed and MANAGE_EXTERNAL_STORAGE is allowed.'
        return $true
    } finally {
        Remove-Item -LiteralPath $temporaryApk -Force -ErrorAction SilentlyContinue
    }
}

function Install-Arisa {
    $temporaryFile = [System.IO.Path]::GetTempFileName()

    try {
        Write-Host "Selected release binary: $($script:ArisaAssetName)"
        Write-Host "Downloading: $($script:ArisaBinaryUrl)"

        try {
            Invoke-WebRequest `
                -Uri $script:ArisaBinaryUrl `
                -OutFile $temporaryFile `
                -UseBasicParsing `
                -ErrorAction Stop
        } catch {
            $statusCode = $null
            if ($_.Exception.Response -and $_.Exception.Response.StatusCode) {
                $statusCode = [int]$_.Exception.Response.StatusCode
            }

            if ($statusCode -eq 404) {
                Write-Host "No Arisa release binary exists for $($script:DeviceArchitecture)."
            } else {
                Write-Host "The Arisa release binary for $($script:DeviceArchitecture) was not found or could not be downloaded."
            }
            Write-Host "URL: $($script:ArisaBinaryUrl)"
            return $false
        }

        Write-Host "Pushing $($script:ArisaAssetName) to $($script:ArisaBinaryPath)..."
        Invoke-SelectedAdb -AdbArguments @('push', $temporaryFile, $script:ArisaBinaryPath) | Out-Host
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'Failed to push Arisa to /data/local/tmp. Check the ADB connection and permissions.'
            return $false
        }

        Invoke-RootAdbShell -Command "chmod 755 '$($script:ArisaBinaryPath)'" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to make $($script:ArisaBinaryPath) executable."
            return $false
        }

        Invoke-RootAdbShell -Command "test -x '$($script:ArisaBinaryPath)'" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Installation verification failed: $($script:ArisaBinaryPath)"
            return $false
        }

        if (-not (Ensure-ArisaConfig)) {
            return $false
        }

        if (-not (Install-FileProvider)) {
            return $false
        }

        Write-Host "Installation completed successfully: $($script:ArisaBinaryPath)"
        return $true
    } finally {
        Remove-Item -LiteralPath $temporaryFile -Force -ErrorAction SilentlyContinue
    }
}

function Remove-Arisa {
    $arisaPid = Get-ArisaPid
    if ($arisaPid) {
        Write-Host 'Stopping Arisa service before removal...'
        Invoke-RootAdbShell -Command "kill -s SIGKILL $arisaPid" | Out-Null
        Start-Sleep -Seconds 1
    }

    Invoke-RootAdbShell -Command "test -e '$($script:ArisaBinaryPath)'" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Arisa is not installed: $($script:ArisaBinaryPath)"
        return $true
    }

    Write-Host "Removing $($script:ArisaBinaryPath)..."
    Invoke-RootAdbShell -Command "rm -f '$($script:ArisaBinaryPath)'" | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to remove $($script:ArisaBinaryPath)."
        return $false
    }

    Invoke-RootAdbShell -Command "test -e '$($script:ArisaBinaryPath)'" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Removal verification failed: $($script:ArisaBinaryPath) still exists."
        return $false
    }

    Write-Host "Arisa was removed successfully: $($script:ArisaBinaryPath)"
    return $true
}

function Remove-AllArisa {
    $failed = $false

    if (-not (Remove-Arisa)) {
        $failed = $true
    }

    Invoke-RootAdbShell -Command "test -e '$($script:ArisaConfigPath)'" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Removing $($script:ArisaConfigPath)..."
        Invoke-RootAdbShell -Command "rm -f '$($script:ArisaConfigPath)'" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to remove $($script:ArisaConfigPath)."
            $failed = $true
        } else {
            Invoke-RootAdbShell -Command "test -e '$($script:ArisaConfigPath)'" | Out-Null
            if ($LASTEXITCODE -eq 0) {
                Write-Host "Removal verification failed: $($script:ArisaConfigPath) still exists."
                $failed = $true
            } else {
                Write-Host "Arisa configuration removed successfully: $($script:ArisaConfigPath)"
            }
        }
    } else {
        Write-Host "Arisa configuration was not found: $($script:ArisaConfigPath)"
    }

    $packagePath = Get-FileProviderPackagePath
    if ($packagePath.StartsWith('package:')) {
        Write-Host "Uninstalling $($script:FileProviderPackage)..."
        Invoke-SelectedAdb -AdbArguments @('uninstall', $script:FileProviderPackage) | Out-Host
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Failed to uninstall $($script:FileProviderPackage)."
            $failed = $true
        }

        $packagePath = Get-FileProviderPackagePath
        if ($packagePath.StartsWith('package:')) {
            Write-Host "Uninstall verification failed: $($script:FileProviderPackage) is still installed."
            $failed = $true
        } else {
            Write-Host "$($script:FileProviderPackage) was uninstalled successfully."
        }
    } else {
        Write-Host "$($script:FileProviderPackage) is not installed."
    }

    if ($failed) {
        Write-Host 'Some Arisa files or components could not be removed.'
        return $false
    }

    Write-Host 'Arisa binary, configuration, and file provider were removed successfully.'
    return $true
}

if (-not (Test-AdbInstalled)) {
    exit 1
}

if (-not (Select-AdbDevice)) {
    exit 1
}

if (-not (Test-SuRootAccess)) {
    exit 1
}

if (-not (Set-ArisaConfigurationForDevice)) {
    exit 1
}

$succeeded = switch ($Action) {
    'status' {
        Show-ArisaStatus
        $true
    }
    'start' {
        Start-Arisa
    }
    'stop' {
        Stop-Arisa
    }
    'install' {
        Install-Arisa
    }
    'config' {
        Set-ArisaConfig
    }
    'remove' {
        Remove-Arisa
    }
    'allremove' {
        Remove-AllArisa
    }
}

if (-not $succeeded) {
    exit 1
}

exit 0
