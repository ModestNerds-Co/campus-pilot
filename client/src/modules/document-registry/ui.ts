export function label(value:string){return value.replace(/_/g," ").replace(/\b\w/g,(letter)=>letter.toUpperCase())}
export function dateValue(value:string|null){if(!value)return "—";return new Intl.DateTimeFormat(undefined,{dateStyle:"medium"}).format(new Date(`${value}T00:00:00`))}
export function dateTime(value:string|null){if(!value)return "—";return new Intl.DateTimeFormat(undefined,{dateStyle:"medium",timeStyle:"short"}).format(new Date(value))}
export function bytes(value:number){if(value<1024)return `${value} B`;if(value<1024*1024)return `${(value/1024).toFixed(1)} KB`;return `${(value/(1024*1024)).toFixed(1)} MB`}
export function tone(value:string):"neutral"|"info"|"success"|"warning"|"danger"{if(["active","filed","executed"].includes(value))return"success";if(["pending","internal","review"].includes(value))return"warning";if(["restricted","destroyed","destroy"].includes(value))return"danger";if(["confidential","approved"].includes(value))return"info";return"neutral"}
