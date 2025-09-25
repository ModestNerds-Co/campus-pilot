//
//  campus-pilot
//  PaymentDetails.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  CreditCard,
  DollarSign,
  Calendar,
  AlertCircle,
  Loader2,
  CheckCircle,
  XCircle,
  Clock,
  Eye,
  X,
  Receipt,
  ExternalLink,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import toast from "react-hot-toast";

interface PaymentDetailsProps {
  applicationId: number;
}

interface PaymentRecord {
  tgapplicationpaymentid: number;
  tgapplicationtypeid: number;
  tgapplicationid: number;
  entitytype: string;
  entityid: number;
  applicationworkflowhistoryid: number;
  paymentmethodlookupid?: number;
  paymentplatform?: string;
  paymentdate?: Date;
  paymenttime?: Date;
  externalreceiptnumber?: string;
  tgfeeid: number;
  feedescription?: string;
  feeamount?: number;
  changedfeeamount?: number;
  currencylookupid?: number;
  tenderedcurrencylookupid?: number;
  amounttendered?: number;
  totalamountdue?: number;
  changereturned?: number;
  taxpercentage?: number;
  paymentstatuslookupid: number;
  instructiontgfinancialdocumentid?: string;
  receipttgfinancialdocumentid?: string;
  paymentgatewayorderid?: string;
  gatewayresult?: string;
  cashiertgsystemuserid?: number;
  paymentinstructionobject?: any;
  paymentreceiptobject?: string;
  referencenumber?: string;
  paidpaymentoptionid?: number;
  paidpaymentoption?: string;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate?: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
}

const getPaymentStatusIcon = (statusId: number) => {
  // You might want to customize these based on actual status lookup values
  if (statusId === 449)
    return <CheckCircle className="w-5 h-5 text-green-600" />;
  if (statusId === 450) return <XCircle className="w-5 h-5 text-red-600" />;
  if (statusId === 451) return <Clock className="w-5 h-5 text-blue-600" />;
  return <AlertCircle className="w-5 h-5 text-gray-600" />;
};

const formatCurrency = (amount: number, currencyCode?: string) => {
  const formatter = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: currencyCode || "ETB",
    minimumFractionDigits: 2,
  });
  return formatter.format(amount);
};

export function PaymentDetails({ applicationId }: PaymentDetailsProps) {
  const [payments, setPayments] = useState<PaymentRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [viewingJson, setViewingJson] = useState<number | null>(null);
  const [jsonData, setJsonData] = useState<any>(null);

  // Fetch payment data
  useEffect(() => {
    const fetchPayments = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getPaymentDetails(applicationId);
        // Convert date strings to Date objects
        const processedData = data.map((payment: any) => ({
          ...payment,
          createdate: new Date(payment.createdate),
          modifieddate: payment.modifieddate
            ? new Date(payment.modifieddate)
            : undefined,
          paymentdate: payment.paymentdate
            ? new Date(payment.paymentdate)
            : undefined,
          paymenttime: payment.paymenttime
            ? new Date(payment.paymenttime)
            : undefined,
        }));
        setPayments(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load payment details";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    if (applicationId) {
      fetchPayments();
    }
  }, [applicationId]);

  const handleViewJson = (payment: PaymentRecord) => {
    setJsonData(payment.paymentinstructionobject);
    setViewingJson(payment.tgapplicationpaymentid);
  };

  const handleCloseJson = () => {
    setViewingJson(null);
    setJsonData(null);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading payment details...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 text-red-500 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            Failed to Load Payment Details
          </h3>
          <p className="text-gray-600 mb-4">{error}</p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h2 className="text-sm font-semibold flex items-center gap-2">
            <CreditCard className="w-4 h-4" />
            Payment Details ({payments.length})
          </h2>
        </div>
      </div>

      {/* Payments List */}
      {payments.length === 0 ? (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <CreditCard className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            💰 No Payment Records Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            No payment transactions have been recorded for this application.
            Payment records will appear here once transactions are processed.
          </p>
          <div className="text-sm text-blue-700 bg-blue-100 px-4 py-2 rounded-lg inline-flex items-center gap-2">
            <span className="text-blue-500">💡</span>
            <span>
              Payment history and transaction details will appear here
              automatically
            </span>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {payments.map((payment) => (
            <div
              key={payment.tgapplicationpaymentid}
              className="bg-white rounded-lg border border-gray-200 p-4"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 bg-gray-100 rounded-lg flex items-center justify-center">
                    {getPaymentStatusIcon(payment.paymentstatuslookupid)}
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-gray-900">
                        {payment.feedescription || "Payment"}
                      </span>
                      <LookupField
                        lookupId={payment.paymentstatuslookupid}
                        format="value"
                        className="px-2 py-0.5 rounded-full font-medium text-xs text-green-700 bg-green-100"
                        fallback="Unknown Status"
                      />
                      {payment.feeamount && (
                        <span className="px-2 py-0.5 rounded-full font-medium text-xs text-blue-700 bg-blue-100">
                          {formatCurrency(payment.feeamount)}
                        </span>
                      )}
                      <span
                        className={cn(
                          "px-2 py-0.5 rounded-full font-medium text-xs",
                          payment.isactive
                            ? "text-green-700 bg-green-100"
                            : "text-gray-700 bg-gray-100",
                        )}
                      >
                        {payment.isactive ? "Active" : "Inactive"}
                      </span>
                    </div>
                    <div className="flex items-center gap-4 mt-1 text-sm text-gray-500">
                      <span>ID: {payment.tgapplicationpaymentid}</span>
                      {payment.paymentdate && (
                        <>
                          <span>•</span>
                          <span>Paid: {formatDate(payment.paymentdate)}</span>
                        </>
                      )}
                      {payment.paymentplatform && (
                        <>
                          <span>•</span>
                          <span>{payment.paymentplatform}</span>
                        </>
                      )}
                      {payment.externalreceiptnumber && (
                        <>
                          <span>•</span>
                          <span className="font-mono text-xs">
                            Receipt: {payment.externalreceiptnumber}
                          </span>
                        </>
                      )}
                      {payment.paymentgatewayorderid && (
                        <>
                          <span>•</span>
                          <span className="font-mono text-xs">
                            Order: {payment.paymentgatewayorderid}
                          </span>
                        </>
                      )}
                    </div>
                    <div className="flex items-center gap-3 text-[10px] text-muted-foreground mt-1">
                      <span>Created: {formatDate(payment.createdate)}</span>
                      {payment.modifieddate && (
                        <>
                          <span>•</span>
                          <span>
                            Modified: {formatDate(payment.modifieddate)}
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {payment.paymentinstructionobject && (
                    <button
                      onClick={() => handleViewJson(payment)}
                      className="p-2 text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
                      title="View Payment Instruction JSON"
                    >
                      <Eye className="w-4 h-4" />
                    </button>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* JSON Viewer Modal */}
      {viewingJson && jsonData && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={handleCloseJson}
        >
          <div
            className="bg-white rounded-lg p-6 max-w-4xl max-h-4xl w-full h-full m-4 flex flex-col"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold">
                Payment Instruction Object - ID: {viewingJson}
              </h3>
              <button
                onClick={handleCloseJson}
                className="compact-button border flex items-center gap-1"
              >
                <X className="w-4 h-4" />
                Close
              </button>
            </div>

            {/* JSON Content */}
            <div className="flex-1 overflow-auto bg-gray-50 rounded border p-4">
              <pre className="text-sm font-mono whitespace-pre-wrap">
                {JSON.stringify(jsonData, null, 2)}
              </pre>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
