// Native IPC deserializes a fresh object on every poll, even when nothing changed.
export function shallowEqual<T extends object>(left:T|undefined,right:T):boolean {
  if(left===right)return true;
  if(!left)return false;
  const keys=Object.keys(right) as (keyof T)[];
  return Object.keys(left).length===keys.length&&keys.every(key=>Object.is(left[key],right[key]));
}
