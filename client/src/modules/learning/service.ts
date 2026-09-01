import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type { ApiEnvelope,CreateLearningResource,CreateLearningSpace,CreateLearningUnit,GovernedFileReference,LearningDownload,LearningFilesResponse,LearningReferenceData,LearningResource,LearningSettings,LearningSpace,LearningSpaceListParams,LearningSpacesResponse,LearningUnit } from "./types";

const BASE="/api/1.0/learning";
async function request<T>(work:()=>Promise<{data:ApiEnvelope<T>}>):Promise<ApiEnvelope<T>>{try{return(await work()).data}catch(error){if(error instanceof AxiosError&&error.response)return error.response.data as ApiEnvelope<T>;throw error}}

export const learningService={
  settings:()=>request<LearningSettings>(()=>httpClient.get(`${BASE}/settings`)),
  updateSettings:(settings:LearningSettings,documentSeriesId:string|null)=>request<LearningSettings>(()=>httpClient.put(`${BASE}/settings`,{document_series_id:documentSeriesId,expected_version:settings.version})),
  references:()=>request<LearningReferenceData>(()=>httpClient.get(`${BASE}/references`)),
  resourceFiles:(search?:string)=>request<LearningFilesResponse>(()=>httpClient.get(`${BASE}/resource-files`,{params:{search:search||undefined,limit:100}})),
  spaces:(params?:LearningSpaceListParams)=>request<LearningSpacesResponse>(()=>httpClient.get(`${BASE}/spaces`,{params})),
  createSpace:(payload:CreateLearningSpace)=>request<LearningSpace>(()=>httpClient.post(`${BASE}/spaces`,payload)),
  space:(id:string)=>request<LearningSpace>(()=>httpClient.get(`${BASE}/spaces/${id}`)),
  updateSpace:(space:LearningSpace,payload:{title:string;summary:string|null})=>request<LearningSpace>(()=>httpClient.put(`${BASE}/spaces/${space.id}`,{...payload,expected_version:space.version})),
  publishSpace:(space:LearningSpace)=>request<LearningSpace>(()=>httpClient.post(`${BASE}/spaces/${space.id}/publish`,{expected_version:space.version})),
  archiveSpace:(space:LearningSpace,reason:string)=>request<LearningSpace>(()=>httpClient.post(`${BASE}/spaces/${space.id}/archive`,{expected_version:space.version,reason})),
  createUnit:(spaceId:string,payload:CreateLearningUnit)=>request<LearningUnit>(()=>httpClient.post(`${BASE}/spaces/${spaceId}/units`,payload)),
  updateUnit:(unit:LearningUnit,payload:CreateLearningUnit)=>request<LearningUnit>(()=>httpClient.put(`${BASE}/units/${unit.id}`,{...payload,expected_version:unit.version})),
  publishUnit:(unit:LearningUnit)=>request<LearningUnit>(()=>httpClient.post(`${BASE}/units/${unit.id}/publish`,{expected_version:unit.version})),
  withdrawUnit:(unit:LearningUnit,reason:string)=>request<LearningUnit>(()=>httpClient.post(`${BASE}/units/${unit.id}/withdraw`,{expected_version:unit.version,reason})),
  createResource:(unitId:string,payload:CreateLearningResource)=>request<LearningResource>(()=>httpClient.post(`${BASE}/units/${unitId}/resources`,payload)),
  uploadResource:(unitId:string,form:FormData)=>request<LearningResource>(()=>httpClient.post(`${BASE}/units/${unitId}/resources/upload`,form)),
  updateResource:(resource:LearningResource,payload:{display_title:string;position:number})=>request<LearningResource>(()=>httpClient.put(`${BASE}/resources/${resource.id}`,{...payload,expected_version:resource.version})),
  publishResource:(resource:LearningResource)=>request<LearningResource>(()=>httpClient.post(`${BASE}/resources/${resource.id}/publish`,{expected_version:resource.version})),
  withdrawResource:(resource:LearningResource,reason:string)=>request<LearningResource>(()=>httpClient.post(`${BASE}/resources/${resource.id}/withdraw`,{expected_version:resource.version,reason})),
  downloadResource:(resourceId:string)=>request<LearningDownload>(()=>httpClient.get(`${BASE}/resources/${resourceId}/download`)),
};

export function responseMessage(response:Pick<ApiEnvelope<unknown>,"issues"|"message">,fallback:string){const issue=response.issues?.[0];if(typeof issue==="string")return issue;if(issue&&typeof issue==="object"&&issue.detail)return issue.detail;return response.message||fallback}
export function fileLabel(file:GovernedFileReference){return `${file.reference} · ${file.title}`}
