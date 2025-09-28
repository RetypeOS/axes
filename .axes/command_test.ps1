param(
    [Parameter(Mandatory=$true)]
    [string]$Comando,

    [Parameter(Mandatory=$true)]
    [int]$Repeticiones
)

# Validación
if ($Repeticiones -le 0) {
    Write-Error "La cantidad de repeticiones debe ser mayor que 0."
    exit
}

$totalTiempo = 0

for ($i = 1; $i -le $Repeticiones; $i++) {
    $tiempo = Measure-Command { Invoke-Expression $Comando }
    $totalTiempo += $tiempo.TotalMilliseconds
    Write-Output "Ejecución ${i}: $($tiempo.TotalMilliseconds) ms"
}

$promedio = $totalTiempo / $Repeticiones
Write-Output "-----------------------------------"
Write-Output "Promedio de ejecución: $promedio ms"