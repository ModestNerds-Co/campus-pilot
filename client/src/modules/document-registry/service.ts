import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type { ActivityResponse,ApiEnvelope,Classification,ClassificationsResponse,DispositionReview,DownloadResponse,FilesResponse,LegalHold,LegalHoldsResponse,NumberingPolicy,RegistryFile,ReviewsResponse,Sensitivity } from "./types";

const BASE="/api/1.0/document-registry";
async function request<T>(work:()=>Promise<{data:ApiEnvelope<T>}>):Promise<ApiEnvelope<T>>{try{return(await work()).data}catch(error){if(error instanceof AxiosError&&error.response)return error.response.data as ApiEnvelope<T>;throw error}}
export type ListParams={page?:number;per_page?:number;search?:string;status?:string;series_id?:string;sensitivity?:string;file_id?:string};
export type ClassificationPayload={code:string;name:string;description:string|null;retention_trigger:"filed"|"closed";retention_period_months:number|null;final_disposition:"review"|"destroy"|"permanent";default_sensitivity:Sensitivity};
export const documentRegistryService={
  numbering:()=>request<NumberingPolicy>(()=>httpClient.get(`${BASE}/numbering-policy`)),
  updateNumbering:(record:NumberingPolicy,payload:{prefix:string;padding:number;next_sequence:number})=>request<NumberingPolicy>(()=>httpClient.put(`${BASE}/numbering-policy`,{...payload,version:record.version})),
  classifications:(params?:ListParams)=>request<ClassificationsResponse>(()=>httpClient.get(`${BASE}/series`,{params})),
  classification:(id:string)=>request<Classification>(()=>httpClient.get(`${BASE}/series/${id}`)),
  createClassification:(payload:ClassificationPayload)=>request<Classification>(()=>httpClient.post(`${BASE}/series`,payload)),
  updateClassification:(record:Classification,payload:ClassificationPayload&{status:"active"|"inactive"})=>request<Classification>(()=>httpClient.put(`${BASE}/series/${record.id}`,{...payload,version:record.version})),
  files:(params?:ListParams)=>request<FilesResponse>(()=>httpClient.get(`${BASE}/files`,{params})),
  file:(id:string)=>request<RegistryFile>(()=>httpClient.get(`${BASE}/files/${id}`)),
  upload:(form:FormData)=>request<RegistryFile>(()=>httpClient.post(`${BASE}/files`,form)),
  updateFile:(record:RegistryFile,payload:{title:string;description:string|null;document_date:string|null;sensitivity:Sensitivity})=>request<RegistryFile>(()=>httpClient.put(`${BASE}/files/${record.id}`,{...payload,version:record.version})),
  reclassify:(record:RegistryFile,payload:{series_id:string;sensitivity:Sensitivity|null;reason:string})=>request<RegistryFile>(()=>httpClient.post(`${BASE}/files/${record.id}/reclassify`,{...payload,version:record.version})),
  close:(record:RegistryFile,reason:string)=>request<RegistryFile>(()=>httpClient.post(`${BASE}/files/${record.id}/close`,{reason,version:record.version})),
  activity:(id:string)=>request<ActivityResponse>(()=>httpClient.get(`${BASE}/files/${id}/activity`)),
  download:(id:string)=>request<DownloadResponse>(()=>httpClient.get(`${BASE}/files/${id}/download`)),
  retentionDue:()=>request<FilesResponse>(()=>httpClient.get(`${BASE}/retention-due`)),
  legalHolds:(params?:ListParams)=>request<LegalHoldsResponse>(()=>httpClient.get(`${BASE}/legal-holds`,{params})),
  legalHold:(id:string)=>request<LegalHold>(()=>httpClient.get(`${BASE}/legal-holds/${id}`)),
  applyLegalHold:(record:RegistryFile,payload:{reference:string|null;reason:string})=>request<LegalHold>(()=>httpClient.post(`${BASE}/files/${record.id}/legal-holds`,{...payload,file_version:record.version})),
  releaseLegalHold:(record:LegalHold,reason:string)=>request<LegalHold>(()=>httpClient.post(`${BASE}/legal-holds/${record.id}/release`,{reason,version:record.version})),
  reviews:(params?:ListParams)=>request<ReviewsResponse>(()=>httpClient.get(`${BASE}/disposition-reviews`,{params})),
  review:(id:string)=>request<DispositionReview>(()=>httpClient.get(`${BASE}/disposition-reviews/${id}`)),
  requestReview:(record:RegistryFile,payload:{recommendation:"retain"|"destroy";proposed_retain_until:string|null;reason:string})=>request<DispositionReview>(()=>httpClient.post(`${BASE}/files/${record.id}/disposition-reviews`,{...payload,file_version:record.version})),
  decideReview:(record:DispositionReview,decision:"approve"|"reject",reason:string)=>request<DispositionReview>(()=>httpClient.post(`${BASE}/disposition-reviews/${record.id}/${decision}`,{reason,version:record.version})),
  executeDestruction:(record:DispositionReview,reason:string)=>request<DispositionReview>(()=>httpClient.post(`${BASE}/disposition-reviews/${record.id}/execute`,{reason,version:record.version})),
};
export function responseMessage(response:ApiEnvelope<unknown>,fallback:string){const first=response.issues?.[0];if(typeof first==="string"&&first.trim())return first;if(first&&typeof first==="object"&&first.detail)return first.detail;return response.message||fallback}
