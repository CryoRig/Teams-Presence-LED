param(
    [string]$ReviewDirectory = 'pcb/outputs/jlc-review-40mm-B-20261112',
    [switch]$ValidateOnly,
    [switch]$FinalizePackage
)

$ErrorActionPreference = 'Stop'
$culture = [Globalization.CultureInfo]::InvariantCulture
$pcbDirectory = Split-Path $PSScriptRoot -Parent
$root = Split-Path $pcbDirectory -Parent
$output = [IO.Path]::GetFullPath($ReviewDirectory, $root)
$review = Join-Path $output 'review'
[xml]$netlist = Get-Content (Join-Path $review 'netlist.xml') -Raw
# jlc-placement.csv comes from jlc_placement.py: centres and angles of JLCPCB's own footprints.
$positions = @(Import-Csv (Join-Path $review 'jlc-placement.csv'))
$nativeRefs = @((Import-Csv (Join-Path $review 'kicad-positions.csv')).Ref)
if (@(Compare-Object $nativeRefs @($positions.Ref)).Count) {
    throw 'JLCPCB placements and native KiCad positions list different references.'
}
$drc = Get-Content (Join-Path $review 'drc-parity.json') -Raw | ConvertFrom-Json
if (@($drc.violations).Count -or @($drc.unconnected_items).Count -or @($drc.schematic_parity).Count) {
    throw 'DRC violations, unrouted items or schematic-parity issues prevent this export.'
}

$choices = @{}
$rows = Get-Content (Join-Path $pcbDirectory 'ORDERING.md') | Where-Object {
    $_ -match '^\| (?:C\d|R\d|F1|L1)'
}
foreach ($row in $rows) {
    $columns = $row.Split('|')
    $references = $columns[1].Trim().Split(',').Trim()
    if ($references.Count -ne [int]$columns[2].Trim()) {
        throw "Quantity mismatch: $row"
    }
    if ($columns[5].Trim() -notmatch '^(C\d+) / (.+) (\S+)$') {
        throw "Missing purchasing identity: $row"
    }
    $choice = [ordered]@{
        lcsc = $Matches[1]
        manufacturer = $Matches[2]
        mpn = $Matches[3]
        status = $columns[6].Trim()
    }
    foreach ($reference in $references) {
        if ($choices.ContainsKey($reference)) { throw "Duplicate choice: $reference" }
        $choices[$reference] = $choice
    }
}
if ($choices.Count -ne 31) { throw 'Expected 31 purchasing-overlay positions.' }

$components = @($netlist.export.components.comp | Where-Object { $_.ref -notmatch '^TP[1-4]$' })
if ($components.Count -ne 47 -or $positions.Count -ne 47) {
    throw 'Expected 47 populated components and 47 native placements.'
}
$positionRefs = @($positions.Ref)
if (($positionRefs | Sort-Object -Unique).Count -ne 47 -or
    @(Compare-Object @($components.ref) $positionRefs).Count) {
    throw 'BOM/netlist/placement reference sets differ.'
}
$details = foreach ($component in $components) {
    $reference = [string]$component.ref
    $fields = @{}
    foreach ($field in @($component.fields.field)) { $fields[[string]$field.name] = $field.InnerText }
    if ($choices.ContainsKey($reference)) {
        $choice = $choices[$reference]
        if ($fields['LCSC'] -ne $choice.lcsc -or $fields['MPN'] -ne $choice.mpn) {
            throw "Schematic and purchasing overlay disagree: $reference"
        }
        $lcsc = $choice.lcsc
        $mpn = $choice.mpn
        $manufacturer = $choice.manufacturer
        $status = $choice.status
    } else {
        $lcsc = $fields['LCSC']
        $mpn = $fields['MPN']
        $manufacturer = ''
        $status = 'Existing schematic identity; assembly preview required'
    }
    if ($lcsc -notmatch '^C\d+$' -or -not $mpn) { throw "Unsourced component: $reference" }
    $placement = @($positions | Where-Object Ref -eq $reference)
    if ($placement.Count -ne 1 -or $placement[0].Side -notin @('top', 'bottom')) {
        throw "Invalid placement: $reference"
    }
    [pscustomobject][ordered]@{
        Designator = $reference
        SchematicValue = [string]$component.value
        Footprint = [string]$component.footprint
        Manufacturer = $manufacturer
        MPN = $mpn
        LCSC = $lcsc
        Status = $status
    }
}
$bom = @($details | Group-Object LCSC, MPN, Footprint | ForEach-Object {
    $part = $_.Group[0]
    [pscustomobject][ordered]@{
        Comment = $part.MPN
        Designator = ($_.Group.Designator -join ',')
        Footprint = $part.Footprint
        'LCSC Part #' = $part.LCSC
    }
})
$cpl = @($positions | ForEach-Object {
    $native = $_
    foreach ($name in @('PosX', 'PosY', 'Rot')) {
        $value = [double]::Parse($native.$name, $culture)
        if ([double]::IsNaN($value) -or [double]::IsInfinity($value)) { throw "Nonfinite $name" }
    }
    [pscustomobject][ordered]@{
        Designator = $native.Ref
        MidX = ([double]::Parse($native.PosX, $culture)).ToString('0.000000', $culture) + 'mm'
        MidY = ([double]::Parse($native.PosY, $culture)).ToString('0.000000', $culture) + 'mm'
        Layer = $native.Side
        Rotation = ([double]::Parse($native.Rot, $culture)).ToString('0.000000', $culture)
    }
})
if ($ValidateOnly) {
    Write-Output "PASS: $($details.Count) sourced positions, $($bom.Count) BOM groups, $($cpl.Count) placements; J1 included; TP1-TP4 excluded."
    return
}
$bom | Export-Csv (Join-Path $output 'BOM-JLC.csv') -NoTypeInformation -Encoding utf8NoBOM
$cpl | Export-Csv (Join-Path $output 'CPL-JLC.csv') -NoTypeInformation -Encoding utf8NoBOM
$details | Export-Csv (Join-Path $review 'purchasing-overlay.csv') -NoTypeInformation -Encoding utf8NoBOM
$sources = @('teams-presence-led.kicad_pcb', 'teams-presence-led.kicad_sch', 'teams-presence-led.kicad_pro', 'ORDERING.md') | ForEach-Object {
    [ordered]@{ path = "pcb/$_"; sha256 = (Get-FileHash (Join-Path $pcbDirectory $_) -Algorithm SHA256).Hash }
}
[ordered]@{
    status = 'JLCPCB engineering review only; not order approved'
    generated = (Get-Date).ToString('o', $culture)
    populatedPositions = $details.Count
    bomGroups = $bom.Count
    placementRows = $cpl.Count
    nativeDrcViolations = @($drc.violations).Count
    nativeUnconnected = @($drc.unconnected_items).Count
    nativeSchematicParity = @($drc.schematic_parity).Count
    coordinates = 'Absolute KiCad origin, mm, Y negated, bottom X not negated; centre and angle of each JLCPCB library footprint fitted to the board pads; bottom angles in JLCPCB mirrored convention'
    sources = $sources
} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $output 'manifest.json') -Encoding utf8NoBOM
Write-Output "Generated review BOM ($($bom.Count) groups) and CPL ($($cpl.Count) placements). Inspect all library rotations in JLC preview before ordering."
if ($FinalizePackage) {
    $fabrication = Join-Path $output 'fabrication'
    $extensions = @('gtl','gbl','gtp','gbp','gto','gbo','gts','gbs','gm1','gbrjob')
    $files = @($extensions | ForEach-Object {
        $suffix = if ($_ -eq 'gbrjob') { '-job.gbrjob' } else {
            @{'gtl'='-F_Cu.gtl';'gbl'='-B_Cu.gbl';'gtp'='-F_Paste.gtp';'gbp'='-B_Paste.gbp';
              'gto'='-F_Silkscreen.gto';'gbo'='-B_Silkscreen.gbo';'gts'='-F_Mask.gts';
              'gbs'='-B_Mask.gbs';'gm1'='-Edge_Cuts.gm1'}[$_]
        }
        Join-Path $fabrication ('teams-presence-led' + $suffix)
    })
    $files += Join-Path $fabrication 'teams-presence-led-PTH.drl'
    $files += Join-Path $fabrication 'teams-presence-led-NPTH.drl'
    foreach ($file in $files) {
        if (-not (Test-Path -LiteralPath $file) -or (Get-Item -LiteralPath $file).Length -eq 0) {
            throw "Missing/empty fabrication file: $file"
        }
    }
    foreach ($file in $files | Where-Object { [IO.Path]::GetExtension($_) -notin @('.drl','.gbrjob') }) {
        if ((Get-Content -LiteralPath $file -Raw) -notmatch '%MOMM\*%') { throw "Not metric Gerber: $file" }
    }
    $plated = Get-Content (Join-Path $fabrication 'teams-presence-led-PTH.drl') -Raw
    $nonplated = Get-Content (Join-Path $fabrication 'teams-presence-led-NPTH.drl') -Raw
    if ($plated -notmatch 'METRIC' -or $nonplated -notmatch 'METRIC' -or
        [regex]::Matches($plated, '(?m)^M15\r?$').Count -ne 4 -or
        [regex]::Matches($nonplated, '(?m)^X[\d.-]+Y[\d.-]+\r?$').Count -ne 2) {
        throw 'Drill units, four plated slots or two NPTH locators are incorrect.'
    }
    foreach ($document in @('README.md','REVIEW.md')) {
        if (-not (Test-Path (Join-Path $output $document))) { throw "Missing $document" }
    }
    $gerberArchive = Join-Path $output 'Gerbers-40mm-review.zip'
    Compress-Archive -LiteralPath $files -DestinationPath $gerberArchive -Force
    $hashFiles = @(Get-ChildItem -LiteralPath $output -Recurse -File | Where-Object Name -ne 'SHA256SUMS.txt')
    $hashFiles | Sort-Object FullName | ForEach-Object {
        (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash + '  ' +
            [IO.Path]::GetRelativePath($output, $_.FullName).Replace('\','/')
    } | Set-Content (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
    $packageFiles = @('Gerbers-40mm-review.zip','BOM-JLC.csv','CPL-JLC.csv','README.md',
        'REVIEW.md','manifest.json','SHA256SUMS.txt','review','fabrication') | ForEach-Object { Join-Path $output $_ }
    Compress-Archive -LiteralPath $packageFiles -DestinationPath ($output + '.zip') -Force
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($gerberArchive)
    try {
        if ($archive.Entries.Count -ne 12 -or @($archive.Entries | Where-Object Length -eq 0).Count) {
            throw 'Gerber archive must contain 9 layers, job metadata and 2 nonempty drill files.'
        }
    } finally { $archive.Dispose() }
    Write-Output "PASS: 12-file metric Gerber/drill archive, $($cpl.Count)-row CPL, $($bom.Count)-group BOM, checksums and full review archive generated at $output.zip"
}