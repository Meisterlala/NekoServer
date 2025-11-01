param (
    [string]$Registry = "registry.meisterlala.dev",
    [switch]$Push = $false
)

$ErrorActionPreference = 'Stop'

# Extract version from Cargo.toml
$cargoContent = Get-Content -Path ".\Cargo.toml" -Raw
if ($cargoContent -match 'version\s*=\s*"([^"]+)"') {
    $version = $matches[1]
} else {
    Write-Error "Could not extract version from Cargo.toml"
    exit 1
}

# Define image tags
$imageName = "neko-server"
$latestTag = "${imageName}:latest"
$versionTag = "${imageName}:$version"
$registryPrefix = "$Registry/"

$buildCommand = "docker buildx build"
$buildCommand += " -t $registryPrefix$latestTag"
$buildCommand += " -t $registryPrefix$versionTag"

if ($Push) {
    $buildCommand += " --platform linux/amd64,linux/arm64 --push"
} else {
    $buildCommand += " --load"
}

$buildCommand += " ."

# Execute the build command
Invoke-Expression $buildCommand
