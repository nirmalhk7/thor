$ErrorActionPreference = "Stop"

Write-Host "THOR_DOWNSTREAM_UNSUPPORTED: Winget packaging is gated until it consumes a signed, provenance-verified Thor Windows browser artifact." -ForegroundColor Red
exit 2
