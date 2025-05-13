# Script to build and push multi-architecture Docker images for NekoServer
# Usage:
#   - Run without parameters: .\build-all.ps1
#   - Specify a custom registry: .\build-all.ps1 -Registry "custom.registry.com"
#   - Push to registry: .\build-all.ps1 -Push
#   - Both custom registry and push: .\build-all.ps1 -Registry "custom.registry.com" -Push

param (
    [string]$Registry = "registry.meisterlala.dev",
    [switch]$Push = $false
)

$ErrorActionPreference = 'Stop'

# Extract version from Cargo.toml
$cargoContent = Get-Content -Path ".\Cargo.toml" -Raw
if ($cargoContent -match 'version\s*=\s*"([^"]+)"') {
    $version = $matches[1]
    Write-Host "Extracted version: $version from Cargo.toml"
} else {
    Write-Error "Could not extract version from Cargo.toml"
    exit 1
}

# Define image name and tags
$imageName = "neko-server"
$latestTag = "${imageName}:latest"
$versionTag = "${imageName}:$version"
$registryPrefix = "$Registry/"

Write-Host "Using registry: $Registry"

# Check if Docker buildx is available
try {
    $buildxVersion = docker buildx version
    Write-Host "Using Docker Buildx: $buildxVersion"
} catch {
    Write-Error "Docker buildx is not available. Please install Docker Desktop with buildx support."
    exit 1
}

# Check if the builder instance exists or create a new one
$builderExists = docker buildx ls | Select-String -Pattern "neko-builder" -Quiet
if (-not $builderExists) {
    Write-Host "Creating new builder instance 'neko-builder'..."
    docker buildx create --name neko-builder --use
} else {
    Write-Host "Using existing builder 'neko-builder'..."
    docker buildx use neko-builder
}

Write-Host "Building Docker image for multiple platforms with version $version..."

$buildCommand = "docker buildx build --platform linux/amd64,linux/arm64"
$buildCommand += " -t $registryPrefix$latestTag"
$buildCommand += " -t $registryPrefix$versionTag"

if ($Push) {
    $buildCommand += " --push"
    Write-Host "Images will be pushed to $Registry"
} else {
    $buildCommand += " --load"
    Write-Host "Images will be built locally (use -Push to push to registry)"
}

$buildCommand += " ."

# Execute the build command
Invoke-Expression $buildCommand

Write-Host "Build completed!"
if ($Push) {
    Write-Host "Images have been pushed to $Registry with tags:"
    Write-Host "  - $registryPrefix$latestTag"
    Write-Host "  - $registryPrefix$versionTag"
} else {
    Write-Host "Images have been built locally with tags:"
    Write-Host "  - $latestTag"
    Write-Host "  - $versionTag"
}