# The provider runs outside an inherited MSIX registry view. Address the current
# user's HKU explicitly; the provider's HKCU can belong to another account.
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false)
$sid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$base=$sid+'\Software\Microsoft\Windows\CurrentVersion\'
$run=$base+'Run'
$approved=$base+'Explorer\StartupApproved\Run'
function Invoke-Registry($method,$key,$extra=@{}) {
 $args=@{hDefKey=[uint32]2147483651;sSubKeyName=$key;sValueName='Whispera'}
 foreach($name in $extra.Keys){$args[$name]=$extra[$name]}
 $result=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName $method -Arguments $args -OperationTimeoutSec 3
 # StdRegProv can return 1 for an absent value in an existing key. Confirm
 # absence explicitly instead of treating every generic error as success.
 if($result.ReturnValue -eq 1 -and $method -in @('GetStringValue','GetBinaryValue','DeleteValue')) {
  $values=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName EnumValues -Arguments @{hDefKey=[uint32]2147483651;sSubKeyName=$key} -OperationTimeoutSec 3
  if($values.ReturnValue -eq 2 -or ($values.ReturnValue -eq 0 -and 'Whispera' -notin @($values.sNames))){return @{ReturnValue=2}}
 }
 if($result.ReturnValue -notin @(0,2)){throw "Windows registry error $($result.ReturnValue)"}
 return $result
}
try {
 $operation=$env:WHISPERA_STARTUP_OPERATION
 if($operation -in @('command','enable')) {
  $result=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName CreateKey -Arguments @{hDefKey=[uint32]2147483651;sSubKeyName=$run} -OperationTimeoutSec 3
  if($result.ReturnValue -ne 0){throw 'Windows did not open startup registration'}
 }
 if($operation -eq 'command') {
  $result=Invoke-Registry 'SetStringValue' $run @{sValue=$env:WHISPERA_STARTUP_COMMAND}
  if($result.ReturnValue -ne 0){throw 'Windows did not save the startup command'}
 } elseif($operation -eq 'enable') {
  $result=Invoke-Registry 'SetStringValue' $run @{sValue=$env:WHISPERA_STARTUP_COMMAND}
  if($result.ReturnValue -ne 0){throw 'Windows did not save the startup command'}
  $result=Invoke-CimMethod -Namespace root/default -ClassName StdRegProv -MethodName CreateKey -Arguments @{hDefKey=[uint32]2147483651;sSubKeyName=$approved} -OperationTimeoutSec 3
  if($result.ReturnValue -ne 0){throw 'Windows did not open startup approval'}
  $result=Invoke-Registry 'SetBinaryValue' $approved @{uValue=[byte[]]@(2,0,0,0,0,0,0,0,0,0,0,0)}
  if($result.ReturnValue -ne 0){throw 'Windows did not enable startup'}
 } elseif($operation -eq 'disable') {
  $null=Invoke-Registry 'DeleteValue' $run
 } elseif($operation -ne 'read'){throw 'Invalid operation'}
 $command=Invoke-Registry 'GetStringValue' $run
 $approval=Invoke-Registry 'GetBinaryValue' $approved
 @{command=$(if($command.ReturnValue -eq 0){$command.sValue}else{$null});approval=$(if($approval.ReturnValue -eq 0){@($approval.uValue)}else{$null})} | ConvertTo-Json -Compress
} catch { [Console]::Error.WriteLine($_.Exception.Message); exit 1 }
