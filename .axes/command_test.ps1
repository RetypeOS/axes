param(
    [Parameter(Mandatory=$true)]
    [string]$Command,

    [Parameter(Mandatory=$true)]
    [int]$Repeat
)

if ($Repeat -le 0) {
    Write-Error "Repetition quantity must be more than 0."
    exit
}

$totalTime = 0
Write-Output "-----------------------------------"
Write-Output "Executing... '$Command'"
Write-Output "-----------------------------------"

for ($i = 1; $i -le $Repeat; $i++) {
    $time = Measure-Command { Invoke-Expression $Command }
    $totalTime += $time.TotalMilliseconds
    Write-Output "Executing ${i}: $($time.TotalMilliseconds) ms"
}

$promediate = $totalTime / $Repeat
Write-Output "-----------------------------------"
Write-Output "Execution Promediate: $promediate ms"
Write-Output "-----------------------------------"
