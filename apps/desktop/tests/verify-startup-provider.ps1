# Exercise the real Windows provider using isolated test keys, never Run.
$ErrorActionPreference='Stop'
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$relative='Software\Whispera\Tests\Startup-'+[guid]::NewGuid().ToString('N')
$fixture=$sid+'\'+$relative
$script=Get-Content (Join-Path $PSScriptRoot '..\src-tauri\src\startup_registry.ps1') -Raw
$original='$base=$sid+''\Software\Microsoft\Windows\CurrentVersion\'''
if(-not $script.Contains($original)){throw 'Fixture substitution no longer matches; refusing to run'}
$script=$script.Replace($original,('$base=$sid+''\'+$relative+'\'''))
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
$executable=Join-Path ([Environment]::GetFolderPath('System')) 'WindowsPowerShell\v1.0\powershell.exe'
$oldOperation=$env:WHISPERA_STARTUP_OPERATION
$oldCommand=$env:WHISPERA_STARTUP_COMMAND
function Run-Provider($operation,$command='') {
 $env:WHISPERA_STARTUP_OPERATION=$operation
 $env:WHISPERA_STARTUP_COMMAND=$command
 $output=& $executable -NoProfile -NonInteractive -EncodedCommand $encoded
 if($LASTEXITCODE -ne 0){throw "Provider failed: $operation"}
 return ($output | ConvertFrom-Json)
}
function Assert($condition,$message){if(-not $condition){throw $message}}
try {
 $empty=Run-Provider 'read'
 Assert ($null -eq $empty.command -and $null -eq $empty.approval) 'Absent registration'
 # Includes spaces, Unicode and shell metacharacters: all must remain literal.
 $command='"C:\Synthetic ñ folder\Whispera $(literal).exe" --autostart'
 $enabled=Run-Provider 'enable' $command
 Assert ($enabled.command -ceq $command) 'Quoted Unicode command round trip'
 Assert (($enabled.approval -join ',') -eq '2,0,0,0,0,0,0,0,0,0,0,0') 'Enable approval'
 $read=Run-Provider 'read'
 Assert ($read.command -ceq $command) 'Persisted command'
 $disabledBytes=[byte[]]@(3,0,0,0,10,0,0,0,0,0,0,0)
 $result=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName SetBinaryValue -Arguments @{hDefKey=[uint32]2147483651;sSubKeyName=$fixture+'\Explorer\StartupApproved\Run';sValueName='Whispera';uValue=$disabledBytes}
 Assert ($result.ReturnValue -eq 0) 'Set isolated disabled approval'
 $repaired=Run-Provider 'command' ($command+'-new')
 Assert ($repaired.command -ceq ($command+'-new')) 'Repair command'
 Assert (($repaired.approval -join ',') -eq ($disabledBytes -join ',')) 'Repair preserves exact approval bytes'
 $enabled=Run-Provider 'enable' $command
 Assert (($enabled.approval -join ',') -eq '2,0,0,0,0,0,0,0,0,0,0,0') 'Explicit enable restores approval'
 $disabled=Run-Provider 'disable'
 Assert ($null -eq $disabled.command) 'Disable removes registration'
 $disabled=Run-Provider 'disable'
 Assert ($null -eq $disabled.command) 'Disable is idempotent'
 Write-Output 'PASS: isolated provider enable/read/repair/disable; quoting, Unicode and approval preservation.'
} finally {
 $env:WHISPERA_STARTUP_OPERATION=$oldOperation
 $env:WHISPERA_STARTUP_COMMAND=$oldCommand
 # Delete only this freshly created test subtree, through the same real view.
 foreach($suffix in @('\Explorer\StartupApproved\Run','\Explorer\StartupApproved','\Explorer','\Run','')) {
  $key=$fixture+$suffix
  if(-not $key.StartsWith($sid+'\Software\Whispera\Tests\Startup-')){throw 'Unsafe fixture cleanup path'}
  $result=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName DeleteKey -Arguments @{hDefKey=[uint32]2147483651;sSubKeyName=$key}
  if($result.ReturnValue -notin @(0,2)){throw "Fixture cleanup failed: $($result.ReturnValue)"}
 }
}
