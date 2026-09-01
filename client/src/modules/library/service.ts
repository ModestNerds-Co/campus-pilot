import { AxiosError } from "axios";
import { httpClient } from "@/lib/http-client";
import type {
  ApiEnvelope,
  CopiesResponse,
  CopyCondition,
  CopyRecord,
  CopyStatus,
  Fine,
  FinesResponse,
  HoldsResponse,
  Hold,
  LibraryReferenceData,
  LibrarySettings,
  Loan,
  LoansResponse,
  Membership,
  MembershipsResponse,
  MembershipStatus,
  TitleDetail,
  TitlePayload,
  TitlesResponse,
} from "./types";

const BASE_URL = "/api/1.0/library";
async function request<T>(
  work: () => Promise<{ data: ApiEnvelope<T> }>,
): Promise<ApiEnvelope<T>> {
  try {
    return (await work()).data;
  } catch (error) {
    if (error instanceof AxiosError && error.response)
      return error.response.data as ApiEnvelope<T>;
    throw error;
  }
}
type ListParams = {
  page?: number;
  per_page?: number;
  search?: string;
  status?: string;
  overdue_only?: boolean;
  membership_id?: string;
};

export const libraryService = {
  settings: () =>
    request<LibrarySettings>(() => httpClient.get(`${BASE_URL}/settings`)),
  updateSettings: (
    payload: Omit<
      LibrarySettings,
      | "next_accession_preview"
      | "fine_currency_code"
      | "fine_currency_minor_units"
      | "updated_at"
    >,
  ) =>
    request<LibrarySettings>(() =>
      httpClient.put(`${BASE_URL}/settings`, payload),
    ),
  references: (search?: string) =>
    request<LibraryReferenceData>(() =>
      httpClient.get(`${BASE_URL}/references`, {
        params: { search: search || undefined },
      }),
    ),
  titles: (params?: ListParams) =>
    request<TitlesResponse>(() =>
      httpClient.get(`${BASE_URL}/titles`, { params }),
    ),
  title: (id: string) =>
    request<TitleDetail>(() => httpClient.get(`${BASE_URL}/titles/${id}`)),
  createTitle: (payload: TitlePayload) =>
    request<TitleDetail>(() => httpClient.post(`${BASE_URL}/titles`, payload)),
  updateTitle: (id: string, version: number, payload: TitlePayload) =>
    request<TitleDetail>(() =>
      httpClient.put(`${BASE_URL}/titles/${id}`, {
        ...payload,
        expected_version: version,
      }),
    ),
  retireTitle: (id: string, version: number) =>
    request<TitleDetail>(() =>
      httpClient.post(`${BASE_URL}/titles/${id}/retire`, {
        expected_version: version,
      }),
    ),
  copies: (
    titleId: string,
    params?: Pick<ListParams, "page" | "per_page" | "status">,
  ) =>
    request<CopiesResponse>(() =>
      httpClient.get(`${BASE_URL}/titles/${titleId}/copies`, { params }),
    ),
  createCopy: (
    titleId: string,
    payload: {
      barcode: string | null;
      location: string | null;
      condition: CopyCondition;
    },
  ) =>
    request<CopyRecord>(() =>
      httpClient.post(`${BASE_URL}/titles/${titleId}/copies`, payload),
    ),
  updateCopy: (
    id: string,
    version: number,
    payload: {
      barcode: string | null;
      location: string | null;
      condition: CopyCondition;
      status: CopyStatus;
    },
  ) =>
    request<CopyRecord>(() =>
      httpClient.put(`${BASE_URL}/copies/${id}`, {
        ...payload,
        expected_version: version,
      }),
    ),
  retireCopy: (id: string, version: number) =>
    request<CopyRecord>(() =>
      httpClient.post(`${BASE_URL}/copies/${id}/retire`, {
        expected_version: version,
      }),
    ),
  members: (params?: ListParams) =>
    request<MembershipsResponse>(() =>
      httpClient.get(`${BASE_URL}/members`, { params }),
    ),
  createMember: (payload: {
    borrower_kind: "learner" | "employee";
    borrower_id: string;
    loan_limit: number | null;
  }) =>
    request<Membership>(() => httpClient.post(`${BASE_URL}/members`, payload)),
  updateMember: (
    id: string,
    version: number,
    status: MembershipStatus,
    loanLimit: number,
  ) =>
    request<Membership>(() =>
      httpClient.put(`${BASE_URL}/members/${id}`, {
        expected_version: version,
        status,
        loan_limit: loanLimit,
      }),
    ),
  loans: (params?: ListParams) =>
    request<LoansResponse>(() =>
      httpClient.get(`${BASE_URL}/loans`, { params }),
    ),
  checkout: (payload: {
    copy_id: string;
    membership_id: string;
    fulfilled_hold_id: string | null;
    checked_out_on: string;
    notes: string | null;
  }) => request<Loan>(() => httpClient.post(`${BASE_URL}/loans`, payload)),
  renewLoan: (loan: Loan, dueOn: string | null) =>
    request<Loan>(() =>
      httpClient.post(`${BASE_URL}/loans/${loan.id}/renew`, {
        expected_version: loan.version,
        due_on: dueOn,
      }),
    ),
  returnLoan: (
    loan: Loan,
    returnedOn: string,
    condition: CopyCondition,
    notes: string | null,
  ) =>
    request<Loan>(() =>
      httpClient.post(`${BASE_URL}/loans/${loan.id}/return`, {
        expected_version: loan.version,
        returned_on: returnedOn,
        copy_condition: condition,
        notes,
      }),
    ),
  markLost: (loan: Loan, reason: string) =>
    request<Loan>(() =>
      httpClient.post(`${BASE_URL}/loans/${loan.id}/lost`, {
        expected_version: loan.version,
        reason,
      }),
    ),
  holds: (params?: ListParams) =>
    request<HoldsResponse>(() =>
      httpClient.get(`${BASE_URL}/holds`, { params }),
    ),
  placeHold: (titleId: string, membershipId: string) =>
    request<Hold>(() =>
      httpClient.post(`${BASE_URL}/holds`, {
        title_id: titleId,
        membership_id: membershipId,
      }),
    ),
  readyHold: (hold: Hold, copyId: string, expiresAt: string) =>
    request<Hold>(() =>
      httpClient.post(`${BASE_URL}/holds/${hold.id}/ready`, {
        expected_version: hold.version,
        copy_id: copyId,
        expires_at: expiresAt,
      }),
    ),
  cancelHold: (hold: Hold, reason: string) =>
    request<Hold>(() =>
      httpClient.post(`${BASE_URL}/holds/${hold.id}/cancel`, {
        expected_version: hold.version,
        reason,
      }),
    ),
  expireHold: (hold: Hold, reason: string) =>
    request<Hold>(() =>
      httpClient.post(`${BASE_URL}/holds/${hold.id}/expire`, {
        expected_version: hold.version,
        reason,
      }),
    ),
  fines: (params?: ListParams) =>
    request<FinesResponse>(() =>
      httpClient.get(`${BASE_URL}/fines`, { params }),
    ),
  assessFine: (loanId: string, kind: "overdue" | "replacement") =>
    request<Fine>(() =>
      httpClient.post(`${BASE_URL}/loans/${loanId}/fines`, { kind }),
    ),
  submitFine: (fine: Fine, billingAccountId: string) =>
    request<Fine>(() =>
      httpClient.post(`${BASE_URL}/fines/${fine.id}/submit-to-fees`, {
        expected_version: fine.version,
        billing_account_id: billingAccountId,
      }),
    ),
  waiveFine: (fine: Fine, reason: string) =>
    request<Fine>(() =>
      httpClient.post(`${BASE_URL}/fines/${fine.id}/waive`, {
        expected_version: fine.version,
        reason,
      }),
    ),
};

export function responseMessage(
  response: Pick<ApiEnvelope<unknown>, "issues" | "message">,
  fallback: string,
) {
  const issue = response.issues?.[0];
  return typeof issue === "string"
    ? issue
    : issue?.detail || response.message || fallback;
}
