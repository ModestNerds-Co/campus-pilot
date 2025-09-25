//
//  campus-pilot
//  Biometrics.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import { useState, useEffect } from "react";
import {
  Fingerprint,
  Plus,
  Edit2,
  CheckCircle,
  XCircle,
  AlertCircle,
  Loader2,
  Eye,
  Download,
  X,
  Users,
  Grid3X3,
  List,
  Camera,
  Scan,
  Zap,
  UserCheck,
  UserX,
  PenTool,
  Upload,
  ImageIcon,
  Crop as CropIcon,
} from "lucide-react";
import { formatDate } from "../../lib/utils";
import { cn } from "../../lib/utils";
import { apiClient } from "../../lib/api";
import { LookupField } from "../LookupField";
import { SearchableSelect } from "../SearchableSelect";
import WSQImageViewer from "../WSQImageViewer";
import FingerprintGrid from "../FingerprintGrid";
import toast from "react-hot-toast";
import ReactCrop, { Crop, PixelCrop } from "react-image-crop";
import "react-image-crop/dist/ReactCrop.css";

// Bulk enroll component
const BulkEnrollButton = ({
  personId,
  onSuccess,
}: {
  personId: string;
  onSuccess: () => void;
}) => {
  const [isLoading, setIsLoading] = useState(false);

  const handleBulkEnroll = async () => {
    if (
      !confirm(
        "Are you sure you want to mark all unenrolled biometrics as enrolled for this person? This action cannot be undone.",
      )
    ) {
      return;
    }

    setIsLoading(true);
    try {
      const response = await apiClient.bulkEnrollBiometrics(personId);
      if (response.success) {
        toast.success(
          `Successfully enrolled ${response.enrolledCount} biometric records`,
        );
        onSuccess();
      } else {
        toast.error("Failed to enroll biometrics");
      }
    } catch (error) {
      console.error("Bulk enroll error:", error);
      toast.error("Failed to enroll biometrics");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <button
      onClick={handleBulkEnroll}
      disabled={isLoading}
      className="compact-button bg-green-600 text-white hover:bg-green-700 disabled:opacity-50"
    >
      {isLoading ? (
        <Loader2 className="w-3 h-3 mr-1 animate-spin" />
      ) : (
        <UserCheck className="w-3 h-3 mr-1" />
      )}
      {isLoading ? "Marking as Enrolled..." : "Mark as Enrolled"}
    </button>
  );
};

// Bulk unenroll component
const BulkUnenrollButton = ({
  personId,
  onSuccess,
}: {
  personId: string;
  onSuccess: () => void;
}) => {
  const [isLoading, setIsLoading] = useState(false);

  const handleBulkUnenroll = async () => {
    if (
      !confirm(
        "Are you sure you want to mark all enrolled biometrics as not enrolled for this person? This action cannot be undone.",
      )
    ) {
      return;
    }

    setIsLoading(true);
    try {
      const response = await apiClient.bulkUnenrollBiometrics(personId);
      if (response.success) {
        toast.success(
          `Successfully unenrolled ${response.unenrolledCount} biometric records`,
        );
        onSuccess();
      } else {
        toast.error("Failed to unenroll biometrics");
      }
    } catch (error) {
      console.error("Bulk unenroll error:", error);
      toast.error("Failed to unenroll biometrics");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <button
      onClick={handleBulkUnenroll}
      disabled={isLoading}
      className="compact-button bg-red-600 text-white hover:bg-red-700 disabled:opacity-50"
    >
      {isLoading ? (
        <Loader2 className="w-3 h-3 mr-1 animate-spin" />
      ) : (
        <UserX className="w-3 h-3 mr-1" />
      )}
      {isLoading ? "Marking as Not Enrolled..." : "Mark as Not Enrolled"}
    </button>
  );
};

// Helper function to get quality color based on percentage
const getQualityColor = (quality: number) => {
  if (quality >= 80)
    return "text-green-700 bg-green-100 px-2 py-0.5 rounded-full font-medium"; // Excellent (80-100%)
  if (quality >= 60)
    return "text-yellow-700 bg-yellow-100 px-2 py-0.5 rounded-full font-medium"; // Good (60-79%)
  if (quality >= 40)
    return "text-orange-700 bg-orange-100 px-2 py-0.5 rounded-full font-medium"; // Fair (40-59%)
  return "text-red-700 bg-red-100 px-2 py-0.5 rounded-full font-medium"; // Poor (0-39%)
};

// Helper function to get icon based on biometric modality
const getModalityIcon = (
  modalitylookupid: number,
  modalityLookupMap: Map<string, number>,
) => {
  const photographId = modalityLookupMap.get("BIOMETRIC_PHOTOGRAPH");
  const fingerprintId = modalityLookupMap.get("BIOMETRIC_FINGERPRINT");
  const irisId = modalityLookupMap.get("BIOMETRIC_IRIS");
  const faceId = modalityLookupMap.get("BIOMETRIC_FACE");
  const signatureId = modalityLookupMap.get("BIOMETRIC_SIGNATURE");

  if (modalitylookupid === photographId) {
    return <Camera className="w-6 h-6 text-gray-600" />;
  } else if (modalitylookupid === fingerprintId) {
    return <Fingerprint className="w-6 h-6 text-gray-600" />;
  } else if (modalitylookupid === irisId) {
    return <Eye className="w-6 h-6 text-gray-600" />;
  } else if (modalitylookupid === faceId) {
    return <UserCheck className="w-6 h-6 text-gray-600" />;
  } else if (modalitylookupid === signatureId) {
    return <PenTool className="w-6 h-6 text-gray-600" />;
  } else {
    // Default fallback
    return <Zap className="w-6 h-6 text-gray-600" />;
  }
};

interface BiometricsProps {
  personId: number;
}

interface BiometricRecord {
  tgpersonbiometricid: number;
  tgpersonid: number;
  modalitylookupid: number;
  imagetypelookupid?: number;
  positionlookupid?: number;
  deviceid?: string;
  imagewidth?: number;
  imageheight?: number;
  captureddpi?: number;
  quality?: number;
  issuccessful: number;
  remark?: string;
  portalrecordstatuslookupid?: number;
  recordstatuslookupid?: number;
  createdate: Date;
  modifieddate: Date;
  tguserauditdetailid: number;
  createdbysystemuserid: number;
  updatedbysystemuserid?: number;
  dataownerlookupid: number;
  isactive: number;
  has_image?: boolean;
}

interface NewBiometricForm {
  imageBase64: string;
  modalitylookupid: number | null;
  positionlookupid: number | null;
  imagetypelookupid: number | null;
  quality: number;
  deviceid: string;
  remark?: string;
}

export function Biometrics({ personId }: BiometricsProps) {
  const [biometrics, setBiometrics] = useState<BiometricRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedBiometric, setSelectedBiometric] = useState<number | null>(
    null,
  );
  const [isAddingNew, setIsAddingNew] = useState(false);
  const [editingBiometric, setEditingBiometric] = useState<number | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [modalityOptions, setModalityOptions] = useState<any[]>([]);
  const [positionOptions, setPositionOptions] = useState<any[]>([]);
  const [imageTypeOptions, setImageTypeOptions] = useState<any[]>([]);
  const [loadingOptions, setLoadingOptions] = useState(true);
  const [viewingImage, setViewingImage] = useState<number | null>(null);
  const [imageData, setImageData] = useState<string | null>(null);
  const [loadingImage, setLoadingImage] = useState(false);
  const [imageDimensions, setImageDimensions] = useState<
    Map<number, { width: number; height: number }>
  >(new Map());
  const [uploadingPhoto, setUploadingPhoto] = useState(false);
  const [showResizeModal, setShowResizeModal] = useState(false);
  const [showCropModal, setShowCropModal] = useState(false);
  const [pendingUpload, setPendingUpload] = useState<{
    originalImage: string;
    dimensions: { width: number; height: number };
  } | null>(null);
  const [pendingCrop, setPendingCrop] = useState<{
    imageUrl: string;
    originalImage: string;
    dimensions: { width: number; height: number };
  } | null>(null);
  const [crop, setCrop] = useState<Crop>({
    unit: "px",
    width: 709,
    height: 945,
    x: 0,
    y: 0,
  });
  const [completedCrop, setCompletedCrop] = useState<PixelCrop | null>(null);

  const [viewMode, setViewMode] = useState<"list" | "grid">("list");
  const [selectedForComparison, setSelectedForComparison] = useState<number[]>(
    [],
  );
  const [positionLookupMap, setPositionLookupMap] = useState<
    Map<number, string>
  >(new Map());
  const [modalityLookupMap, setModalityLookupMap] = useState<
    Map<string, number>
  >(new Map());

  const [newBiometricForm, setNewBiometricForm] = useState<NewBiometricForm>({
    imageBase64: "",
    modalitylookupid: null,
    positionlookupid: null,
    imagetypelookupid: null,
    quality: 0,
    deviceid: "",
  });

  // Load finger position and modality lookups
  useEffect(() => {
    const loadLookups = async () => {
      try {
        const [positionLookups, modalityLookups] = await Promise.all([
          apiClient.getLookupsByType("FINGER_POSITION"),
          apiClient.getLookupsByType("BIOMETRIC_MODALITY"),
        ]);

        const positionMap = new Map<number, string>();
        positionLookups.forEach((lookup: any) => {
          positionMap.set(lookup.tglookupid, lookup.lookupvalue);
        });
        setPositionLookupMap(positionMap);

        const modalityMap = new Map<string, number>();
        modalityLookups.forEach((lookup: any) => {
          modalityMap.set(lookup.lookupvalue, lookup.tglookupid);
        });
        setModalityLookupMap(modalityMap);
      } catch (error) {
        console.error("Failed to load lookups:", error);
      }
    };

    loadLookups();
  }, []);

  // Function to calculate image dimensions from base64 data
  const calculateImageDimensions = async (biometricId: number) => {
    // Skip if already calculated
    if (imageDimensions.has(biometricId)) return;

    try {
      const imageResponse = await apiClient.getBiometricImage(
        personId,
        biometricId,
      );
      if (imageResponse.image && !imageResponse.isWSQ) {
        const img = new Image();
        img.onload = () => {
          setImageDimensions(
            (prev) =>
              new Map(
                prev.set(biometricId, {
                  width: img.naturalWidth,
                  height: img.naturalHeight,
                }),
              ),
          );
        };
        img.src = imageResponse.image;
      }
    } catch (error) {
      // Silently fail for dimension calculation
      console.debug(
        "Could not calculate dimensions for biometric:",
        biometricId,
      );
    }
  };

  // Fetch biometric data
  useEffect(() => {
    const fetchBiometrics = async () => {
      try {
        setIsLoading(true);
        setError(null);

        const data = await apiClient.getBiometrics(personId);
        // Convert date strings to Date objects
        const processedData = data.map((bio: any) => ({
          ...bio,
          createdate: new Date(bio.createdate),
          modifieddate: new Date(bio.modifieddate),
        }));
        setBiometrics(processedData);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to load biometric data";
        setError(errorMessage);
        toast.error(errorMessage);
      } finally {
        setIsLoading(false);
      }
    };

    fetchBiometrics();
  }, [personId]);

  // Calculate dimensions when biometrics and modality lookup are ready
  useEffect(() => {
    if (
      biometrics.length > 0 &&
      modalityLookupMap.has("BIOMETRIC_PHOTOGRAPH")
    ) {
      biometrics.forEach((bio) => {
        if (
          bio.has_image &&
          bio.modalitylookupid === modalityLookupMap.get("BIOMETRIC_PHOTOGRAPH")
        ) {
          calculateImageDimensions(bio.tgpersonbiometricid);
        }
      });
    }
  }, [biometrics, modalityLookupMap]);

  // Fetch lookup options
  useEffect(() => {
    const fetchLookupOptions = async () => {
      try {
        setLoadingOptions(true);
        const [modalityData, positionData, imageTypeData] = await Promise.all([
          apiClient.getLookupsByType("BIOMETRIC_MODALITY"),
          apiClient.getLookupsByType("BIOMETRIC_POSITION"),
          apiClient.getLookupsByType("BIOMETRIC_IMAGE_TYPE"),
        ]);

        setModalityOptions(
          modalityData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setPositionOptions(
          positionData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
        setImageTypeOptions(
          imageTypeData.map((lookup) => ({
            id: lookup.tglookupid,
            value: lookup.lookupvalue,
            label: lookup.lookupdescription || lookup.lookupvalue,
          })),
        );
      } catch (err) {
        console.error("Failed to load lookup options:", err);
      } finally {
        setLoadingOptions(false);
      }
    };

    fetchLookupOptions();
  }, []);

  const handleAddNew = async () => {
    setIsSubmitting(true);
    try {
      const biometricData = {
        tgpersonid: personId,
        imageBase64: newBiometricForm.imageBase64,
      };

      const result = await apiClient.addBiometric(biometricData);

      const newBiometric: BiometricRecord = {
        tgpersonbiometricid: result.tgpersonbiometricid,
        tgpersonid: personId,
        modalitylookupid: modalityLookupMap.get("BIOMETRIC_PHOTOGRAPH") || 0,
        positionlookupid: undefined,
        imagetypelookupid: undefined,
        deviceid: undefined,
        quality: undefined,
        issuccessful: 0, // Default to false, will be set to true when enrolled in ABIS
        remark: undefined,
        createdate: new Date(),
        modifieddate: new Date(),
        tguserauditdetailid: 1,
        createdbysystemuserid: 1,
        updatedbysystemuserid: undefined,
        dataownerlookupid: 4,
        isactive: 1,
        has_image: !!newBiometricForm.imageBase64,
      };

      setBiometrics((prev) => [newBiometric, ...prev]);
      setIsAddingNew(false);
      setNewBiometricForm({
        imageBase64: "",
        modalitylookupid: null,
        positionlookupid: null,
        imagetypelookupid: null,
        quality: 0,
        deviceid: "",
      });

      toast.success("Biometric record added successfully");
    } catch (error) {
      toast.error("Failed to add biometric record");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleVoid = async (id: number) => {
    const confirmed = window.confirm(
      "Are you sure you want to void this biometric record?",
    );
    if (!confirmed) return;

    try {
      await apiClient.voidBiometric(personId, id);
      setBiometrics((prev) =>
        prev.map((bio) =>
          bio.tgpersonbiometricid === id
            ? { ...bio, isactive: 0, modifieddate: new Date() }
            : bio,
        ),
      );
      toast.success("Biometric record voided successfully");
    } catch (error) {
      toast.error("Failed to void biometric record");
    }
  };

  const handleMarkEnrolled = async (id: number) => {
    const confirmed = window.confirm(
      "Mark this biometric record as enrolled in ABIS?",
    );
    if (!confirmed) return;

    try {
      await apiClient.markBiometricEnrolled(personId, id);
      setBiometrics((prev) =>
        prev.map((bio) =>
          bio.tgpersonbiometricid === id
            ? { ...bio, issuccessful: 1, modifieddate: new Date() }
            : bio,
        ),
      );
      toast.success("Biometric record marked as enrolled successfully");
    } catch (error) {
      toast.error("Failed to mark biometric as enrolled");
    }
  };

  const handleMarkNotEnrolled = async (id: number) => {
    const confirmed = window.confirm(
      "Mark this biometric record as not enrolled in ABIS?",
    );
    if (!confirmed) return;

    try {
      await apiClient.markBiometricNotEnrolled(personId, id);
      setBiometrics((prev) =>
        prev.map((bio) =>
          bio.tgpersonbiometricid === id
            ? { ...bio, issuccessful: 0, modifieddate: new Date() }
            : bio,
        ),
      );
      toast.success("Biometric record marked as not enrolled successfully");
    } catch (error) {
      toast.error("Failed to mark biometric as not enrolled");
    }
  };

  const handleViewImage = async (biometricId: number) => {
    // Find the biometric record to check modality type
    const biometricRecord = biometrics.find(
      (b) => b.tgpersonbiometricid === biometricId,
    );

    // Check if this is a fingerprint biometric by checking the lookup value
    if (biometricRecord) {
      try {
        const lookupResponse = await apiClient.getLookupValue(
          biometricRecord.modalitylookupid,
        );
        if (
          lookupResponse &&
          (lookupResponse.lookupvalue === "BIOMETRIC_FINGERPRINT" ||
            lookupResponse.lookupvalue === "FINGERPRINT")
        ) {
          toast.error("WSQ fingerprint images cannot be viewed here");
          return;
        }
      } catch (error) {
        console.warn(
          "Could not determine modality type, proceeding with view:",
          error,
        );
      }
    }

    try {
      setLoadingImage(true);
      setViewingImage(biometricId);

      const imageResponse = await apiClient.getBiometricImage(
        personId,
        biometricId,
      );

      if (imageResponse.image) {
        // Store the raw image data - for non-fingerprint images
        setImageData(imageResponse.image);
      } else {
        throw new Error("No image data received");
      }
    } catch (error) {
      toast.error("Failed to load biometric image");
      setViewingImage(null);
    } finally {
      setLoadingImage(false);
    }
  };

  const handleCloseImage = () => {
    setViewingImage(null);
    setImageData(null);
  };

  const handleDownloadImage = async () => {
    if (viewingImage) {
      try {
        const imageResponse = await apiClient.getBiometricImage(
          personId,
          viewingImage,
        );

        if (imageResponse.image) {
          const link = document.createElement("a");

          // Set appropriate filename and data based on format
          if (imageResponse.format === "wsq") {
            // WSQ format - save as .wsq file
            const blob = new Blob([imageResponse.image], {
              type: "application/octet-stream",
            });
            link.href = URL.createObjectURL(blob);
            link.download = `biometric-${viewingImage}.wsq`;
          } else {
            // Base64 format - save as image
            link.href = imageResponse.image.startsWith("data:")
              ? imageResponse.image
              : `data:image/jpeg;base64,${imageResponse.image}`;
            link.download = `biometric-${viewingImage}.jpg`;
          }

          document.body.appendChild(link);
          link.click();
          document.body.removeChild(link);

          toast.success("Biometric image downloaded successfully");
        }
      } catch (error) {
        toast.error("Failed to download biometric image");
      }
    }
  };

  const convertImageToJpg = async (file: File): Promise<string> => {
    return new Promise((resolve, reject) => {
      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d");
      const img = new Image();

      img.onload = () => {
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;

        // Fill with white background (important for transparent PNGs)
        if (ctx) {
          ctx.fillStyle = "white";
          ctx.fillRect(0, 0, canvas.width, canvas.height);
          ctx.drawImage(img, 0, 0);
        }

        // Convert to JPG with 0.9 quality
        const jpgDataUrl = canvas.toDataURL("image/jpeg", 0.9);
        const base64 = jpgDataUrl.split(",")[1];
        resolve(base64);
      };

      img.onerror = reject;
      img.src = URL.createObjectURL(file);
    });
  };

  // Helper function to check if image has correct aspect ratio (3:4)
  const hasCorrectAspectRatio = (width: number, height: number): boolean => {
    const aspectRatio = width / height;
    const targetRatio = 3 / 4; // 0.75
    const tolerance = 0.05; // Allow 5% tolerance
    return Math.abs(aspectRatio - targetRatio) <= tolerance;
  };

  // Function to crop image using canvas
  const cropImage = async (
    imageSrc: string,
    crop: PixelCrop,
  ): Promise<string> => {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement("canvas");
        const ctx = canvas.getContext("2d");

        if (!ctx) {
          reject(new Error("Canvas context not available"));
          return;
        }

        // Set canvas size to target dimensions
        canvas.width = 709;
        canvas.height = 945;

        // Fill with white background
        ctx.fillStyle = "white";
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // Draw cropped image scaled to fit 709x945
        ctx.drawImage(
          img,
          crop.x,
          crop.y,
          crop.width,
          crop.height, // Source rectangle
          0,
          0,
          canvas.width,
          canvas.height, // Destination rectangle
        );

        // Convert to base64
        const croppedBase64 = canvas.toDataURL("image/jpeg", 0.9).split(",")[1];
        resolve(croppedBase64);
      };
      img.onerror = reject;
      img.src = imageSrc;
    });
  };

  const handleNewBiometricPhotoUpload = async (file: File) => {
    try {
      setUploadingPhoto(true);

      // Convert to JPG format and get base64
      const base64 = await convertImageToJpg(file);

      // Get image dimensions
      const dimensions = await new Promise<{ width: number; height: number }>(
        (resolve, reject) => {
          const img = new Image();
          img.onload = () =>
            resolve({ width: img.naturalWidth, height: img.naturalHeight });
          img.onerror = reject;
          img.src = URL.createObjectURL(file);
        },
      );

      // Check aspect ratio first (3:4)
      if (!hasCorrectAspectRatio(dimensions.width, dimensions.height)) {
        // Image doesn't have correct aspect ratio, show cropper
        const imageUrl = URL.createObjectURL(file);
        setPendingCrop({
          imageUrl,
          originalImage: base64,
          dimensions,
        });
        setShowCropModal(true);
        setUploadingPhoto(false);
        return;
      }

      // Check if image meets minimum dimensions
      if (dimensions.width < 709 || dimensions.height < 945) {
        setPendingUpload({
          originalImage: base64,
          dimensions,
        });
        setShowResizeModal(true);
        setUploadingPhoto(false);
        return;
      }

      // Image meets requirements, set directly to form
      setNewBiometricForm((prev) => ({ ...prev, imageBase64: base64 }));
      setUploadingPhoto(false);
      toast.success("Photo uploaded successfully");
    } catch (error) {
      console.error("Photo upload error:", error);
      toast.error("Failed to process photo upload");
      setUploadingPhoto(false);
    }
  };

  const resizeImage = async (
    base64: string,
    targetWidth: number,
    targetHeight: number,
  ): Promise<string> => {
    return new Promise((resolve) => {
      const canvas = document.createElement("canvas");
      const ctx = canvas.getContext("2d")!;
      const img = new Image();

      img.onload = () => {
        canvas.width = targetWidth;
        canvas.height = targetHeight;

        // Draw and resize image
        ctx.drawImage(img, 0, 0, targetWidth, targetHeight);

        // Convert back to base64
        const resizedBase64 = canvas.toDataURL("image/jpeg", 0.9).split(",")[1];
        resolve(resizedBase64);
      };

      img.src = `data:image/jpeg;base64,${base64}`;
    });
  };

  const handleResizeConfirmation = async (resize: boolean) => {
    if (!pendingUpload) return;

    try {
      setUploadingPhoto(true);

      let finalBase64: string;
      if (resize) {
        // Resize image to 709x945
        finalBase64 = await resizeImage(pendingUpload.originalImage, 709, 945);
      } else {
        // Use original image as-is
        finalBase64 = pendingUpload.originalImage;
      }

      // Set the base64 data to the new biometric form
      setNewBiometricForm((prev) => ({ ...prev, imageBase64: finalBase64 }));
      toast.success("Photo uploaded successfully");
    } catch (error) {
      console.error("Upload confirmation error:", error);
      toast.error("Failed to process photo");
    } finally {
      setUploadingPhoto(false);
      setShowResizeModal(false);
      setPendingUpload(null);
    }
  };

  const handleCropConfirmation = async () => {
    if (!pendingCrop || !completedCrop) return;

    try {
      setUploadingPhoto(true);

      // Crop the image
      const croppedBase64 = await cropImage(
        pendingCrop.imageUrl,
        completedCrop,
      );

      // Set the cropped image to the form
      setNewBiometricForm((prev) => ({ ...prev, imageBase64: croppedBase64 }));
      toast.success("Photo cropped and uploaded successfully");
    } catch (error) {
      console.error("Crop confirmation error:", error);
      toast.error("Failed to process cropped image");
    } finally {
      setUploadingPhoto(false);
      setShowCropModal(false);
      setPendingCrop(null);
      setCompletedCrop(null);
    }
  };

  const handleCropCancel = () => {
    if (pendingCrop) {
      URL.revokeObjectURL(pendingCrop.imageUrl);
    }
    setShowCropModal(false);
    setPendingCrop(null);
    setCompletedCrop(null);
  };

  const MetaStrip = ({ record }: { record: BiometricRecord }) => (
    <div className="flex items-center gap-3 text-[10px] text-muted-foreground">
      <span>ID: {record.tgpersonbiometricid}</span>
      <span>•</span>
      <span>{formatDate(record.createdate)}</span>
      <span>•</span>
      <span>{formatDate(record.modifieddate)}</span>
      <span>•</span>
      <span
        className={cn(
          "badge text-[9px]",
          record.isactive ? "badge-success" : "badge-neutral",
        )}
      >
        {record.isactive ? "Active" : "Inactive"}
      </span>
    </div>
  );

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
          <span className="text-gray-600">Loading biometric data...</span>
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
            Failed to Load Biometrics
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

  const fingerprintModalityId = modalityLookupMap.get("BIOMETRIC_FINGERPRINT");
  const fingerprintRecords = biometrics.filter(
    (bio) => bio.modalitylookupid === fingerprintModalityId,
  );

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h2 className="text-sm font-semibold flex items-center gap-2">
            <Fingerprint className="w-4 h-4" />
            Biometric Records ({biometrics.length})
          </h2>
          {fingerprintRecords.length > 0 && (
            <div className="flex items-center gap-1 bg-gray-100 rounded-lg p-1">
              <button
                onClick={() => setViewMode("list")}
                className={cn(
                  "p-1.5 rounded transition-colors",
                  viewMode === "list"
                    ? "bg-white text-gray-900 shadow-sm"
                    : "text-gray-500 hover:text-gray-700",
                )}
                title="List View"
              >
                <List className="w-4 h-4" />
              </button>
              <button
                onClick={() => setViewMode("grid")}
                className={cn(
                  "p-1.5 rounded transition-colors",
                  viewMode === "grid"
                    ? "bg-white text-gray-900 shadow-sm"
                    : "text-gray-500 hover:text-gray-700",
                )}
                title="Grid View"
              >
                <Grid3X3 className="w-4 h-4" />
              </button>
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          {selectedForComparison.length > 0 && (
            <button
              onClick={() => setSelectedForComparison([])}
              className="compact-button border text-gray-600"
            >
              Clear Selection ({selectedForComparison.length})
            </button>
          )}
          {/* Only show BulkEnrollButton if there are biometrics and some are not successful */}
          {biometrics.length > 0 &&
            biometrics.some((bio) => !bio.issuccessful) && (
              <BulkEnrollButton
                personId={personId.toString()}
                onSuccess={() => {
                  // Refetch biometrics data instead of reloading the page
                  const fetchBiometrics = async () => {
                    try {
                      const data = await apiClient.getBiometrics(personId);
                      const processedData = data.map((bio: any) => ({
                        ...bio,
                        createdate: new Date(bio.createdate),
                        modifieddate: new Date(bio.modifieddate),
                      }));
                      setBiometrics(processedData);
                    } catch (err) {
                      console.error("Failed to refresh biometrics:", err);
                    }
                  };
                  fetchBiometrics();
                }}
              />
            )}
          {/* Only show BulkUnenrollButton if there are biometrics and some are successful */}
          {biometrics.length > 0 &&
            biometrics.some((bio) => bio.issuccessful) && (
              <BulkUnenrollButton
                personId={personId.toString()}
                onSuccess={() => {
                  // Refetch biometrics data instead of reloading the page
                  const fetchBiometrics = async () => {
                    try {
                      const data = await apiClient.getBiometrics(personId);
                      const processedData = data.map((bio: any) => ({
                        ...bio,
                        createdate: new Date(bio.createdate),
                        modifieddate: new Date(bio.modifieddate),
                      }));
                      setBiometrics(processedData);
                    } catch (err) {
                      console.error("Failed to refresh biometrics:", err);
                    }
                  };
                  fetchBiometrics();
                }}
              />
            )}
          <button
            onClick={() => setIsAddingNew(true)}
            className="compact-button bg-primary text-white"
          >
            <Plus className="w-3 h-3 mr-1" />
            Add Biometric
          </button>
        </div>
      </div>

      {/* Add New Form */}
      {isAddingNew && (
        <div className="bg-card border rounded-lg p-4 space-y-4">
          <h3 className="text-sm font-semibold">Add New Biometric Record</h3>

          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-2">
                Photograph Data *
              </label>

              {/* File Upload Option */}
              <div className="border-2 border-dashed border-gray-300 rounded-lg p-6 text-center mb-4">
                <input
                  type="file"
                  id="photo-file-input"
                  accept="image/jpeg,image/jpg,image/png,image/gif,image/webp"
                  className="hidden"
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) {
                      handleNewBiometricPhotoUpload(file);
                    }
                  }}
                />
                <div className="space-y-2">
                  <Upload className="w-8 h-8 mx-auto text-gray-400" />
                  <div>
                    <button
                      type="button"
                      onClick={() => {
                        const input = document.getElementById(
                          "photo-file-input",
                        ) as HTMLInputElement;
                        input?.click();
                      }}
                      disabled={uploadingPhoto}
                      className="text-blue-600 hover:text-blue-700 font-medium disabled:opacity-50"
                    >
                      {uploadingPhoto ? "Processing..." : "Choose photo file"}
                    </button>
                    <span className="text-gray-500"> or drag and drop</span>
                  </div>
                  <p className="text-xs text-gray-500">
                    PNG, JPG, GIF, WebP up to 10MB. Automatically converted to
                    JPG format. Minimum 709×945px, 3:4 aspect ratio required.
                  </p>
                </div>
              </div>

              {/* Divider */}
              <div className="relative">
                <div className="absolute inset-0 flex items-center">
                  <div className="w-full border-t border-gray-300" />
                </div>
                <div className="relative flex justify-center text-sm">
                  <span className="px-2 bg-white text-gray-500">or</span>
                </div>
              </div>

              {/* Manual Base64 Input */}
              <div className="mt-4">
                <label className="block text-xs font-medium text-gray-600 mb-1">
                  Paste Base64 Data Manually
                </label>
                <textarea
                  value={newBiometricForm.imageBase64}
                  onChange={(e) =>
                    setNewBiometricForm((prev) => ({
                      ...prev,
                      imageBase64: e.target.value,
                    }))
                  }
                  className="w-full p-2 text-xs border rounded min-h-[120px] font-mono"
                  placeholder="Paste base64 encoded photograph data here..."
                />
              </div>

              {/* Preview if data exists */}
              {newBiometricForm.imageBase64 && (
                <div className="mt-2 p-2 bg-green-50 border border-green-200 rounded text-xs text-green-700">
                  ✓ Photo data loaded (
                  {Math.round(newBiometricForm.imageBase64.length / 1024)}KB)
                </div>
              )}
            </div>
          </div>

          <div className="flex justify-end gap-2">
            <button
              onClick={() => {
                setIsAddingNew(false);
                setNewBiometricForm({
                  modalitylookupid: null,
                  positionlookupid: null,
                  imagetypelookupid: null,
                  deviceid: "",
                  quality: 80,
                  remark: "",
                  imageBase64: "",
                });
              }}
              className="compact-button border"
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button
              onClick={handleAddNew}
              className="compact-button bg-primary text-white"
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  Adding...
                </>
              ) : (
                "Add Record"
              )}
            </button>
          </div>
        </div>
      )}

      {/* Fingerprint Grid View */}
      {viewMode === "grid" && fingerprintRecords.length > 0 && (
        <div className="bg-white rounded-lg border p-6">
          <FingerprintGrid
            personId={personId}
            personName="Current Person"
            fingerprints={fingerprintRecords.map((fp) => ({
              ...fp,
              position:
                positionLookupMap.get(fp.positionlookupid || 0) || "UNKNOWN",
              imageData: "", // Will be loaded on demand
            }))}
          />
        </div>
      )}

      {/* Biometric List */}
      {viewMode === "list" && biometrics.length === 0 ? (
        <div className="text-center py-12 bg-gradient-to-br from-blue-50 to-indigo-50 rounded-lg border-2 border-dashed border-blue-200">
          <Fingerprint className="w-16 h-16 text-blue-300 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            🔐 No Biometric Records Found
          </h3>
          <p className="text-gray-600 mb-4 max-w-md mx-auto">
            This person has no biometric data captured yet. Biometrics include
            fingerprints, facial photos, and other identification data.
          </p>
          <div className="flex flex-col items-center gap-3">
            <button
              onClick={() => setIsAddingNew(true)}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 flex items-center gap-2"
            >
              <Plus className="w-4 h-4" />
              Capture First Biometric
            </button>
            <div className="text-sm text-blue-700 bg-blue-100 px-3 py-2 rounded-lg inline-flex items-center gap-2">
              <span className="text-blue-500">💡</span>
              <span>
                Fingerprints and photos will appear here once captured
              </span>
            </div>
          </div>
        </div>
      ) : viewMode === "list" && biometrics.length > 0 ? (
        <div className="space-y-3">
          {biometrics.map((biometric) => (
            <div
              key={biometric.tgpersonbiometricid}
              className="bg-white rounded-lg border border-gray-200 p-4"
            >
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                  <div className="w-12 h-12 bg-gray-100 rounded-lg flex items-center justify-center">
                    {getModalityIcon(
                      biometric.modalitylookupid,
                      modalityLookupMap,
                    )}
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <LookupField
                        lookupId={biometric.modalitylookupid}
                        format="value"
                        className="font-medium text-gray-900"
                        fallback="Unknown Modality"
                      />
                      {biometric.positionlookupid && (
                        <>
                          <span className="text-gray-400">•</span>
                          <LookupField
                            lookupId={biometric.positionlookupid}
                            format="value"
                            className="text-sm text-gray-600"
                            fallback="Unknown Position"
                          />
                        </>
                      )}
                    </div>
                    <div className="flex items-center gap-4 mt-1 text-sm text-gray-500">
                      <span>ID: {biometric.tgpersonbiometricid}</span>
                      {biometric.quality && (
                        <span className={getQualityColor(biometric.quality)}>
                          Quality: {biometric.quality}%
                        </span>
                      )}
                      {imageDimensions.has(biometric.tgpersonbiometricid) && (
                        <>
                          <span>
                            {
                              imageDimensions.get(biometric.tgpersonbiometricid)
                                ?.width
                            }{" "}
                            ×{" "}
                            {
                              imageDimensions.get(biometric.tgpersonbiometricid)
                                ?.height
                            }
                            px
                          </span>
                          <span
                            className={cn(
                              "px-2 py-0.5 rounded-full font-medium text-xs",
                              hasCorrectAspectRatio(
                                imageDimensions.get(
                                  biometric.tgpersonbiometricid,
                                )?.width || 0,
                                imageDimensions.get(
                                  biometric.tgpersonbiometricid,
                                )?.height || 0,
                              )
                                ? "text-green-700 bg-green-100"
                                : "text-orange-700 bg-orange-100",
                            )}
                          >
                            {(() => {
                              const dims = imageDimensions.get(
                                biometric.tgpersonbiometricid,
                              );
                              if (dims) {
                                const ratio = (
                                  dims.width / dims.height
                                ).toFixed(2);
                                return `${ratio}:1`;
                              }
                              return "Unknown";
                            })()}
                          </span>
                        </>
                      )}
                      <span
                        className={cn(
                          "px-2 py-0.5 rounded-full font-medium text-xs",
                          biometric.issuccessful
                            ? "text-green-700 bg-green-100"
                            : "text-red-700 bg-red-100",
                        )}
                      >
                        {biometric.issuccessful ? "Enrolled" : "Not Enrolled"}
                      </span>
                      <span>Created: {formatDate(biometric.createdate)}</span>
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {biometric.has_image && (
                    <button
                      onClick={() =>
                        handleViewImage(biometric.tgpersonbiometricid)
                      }
                      className="p-2 text-gray-500 hover:text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
                      title="View Image"
                    >
                      <Eye className="w-4 h-4" />
                    </button>
                  )}
                  {!biometric.issuccessful && (
                    <button
                      onClick={() =>
                        handleMarkEnrolled(biometric.tgpersonbiometricid)
                      }
                      className="p-2 text-gray-500 hover:text-green-600 hover:bg-green-50 rounded-lg transition-colors"
                      title="Mark as Enrolled"
                    >
                      <CheckCircle className="w-4 h-4" />
                    </button>
                  )}
                  {biometric.issuccessful && (
                    <button
                      onClick={() =>
                        handleMarkNotEnrolled(biometric.tgpersonbiometricid)
                      }
                      className="p-2 text-gray-500 hover:text-red-600 hover:bg-red-50 rounded-lg transition-colors"
                      title="Mark as Not Enrolled"
                    >
                      <UserX className="w-4 h-4" />
                    </button>
                  )}
                  <button
                    onClick={() => handleVoid(biometric.tgpersonbiometricid)}
                    className="p-2 text-gray-500 hover:text-red-600 hover:bg-red-50 rounded-lg transition-colors"
                    title="Void Record"
                  >
                    <XCircle className="w-4 h-4" />
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : null}

      {/* Edit Modal Placeholder */}
      {editingBiometric && (
        <div
          className="modal-overlay"
          onClick={() => setEditingBiometric(null)}
        >
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h3 className="text-sm font-semibold mb-4">
              Edit Biometric Record
            </h3>
            <p className="text-xs text-muted-foreground">
              Biometric editing form would go here
            </p>
            <div className="flex justify-end gap-2 mt-4">
              <button
                onClick={() => setEditingBiometric(null)}
                className="compact-button border border-gray-200"
              >
                Cancel
              </button>
              <button className="compact-button bg-primary text-white">
                Save Changes
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Image Viewer Modal */}
      {viewingImage && (
        <div
          className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50"
          onClick={handleCloseImage}
        >
          <div
            className="bg-white rounded-lg p-4 max-w-4xl max-h-4xl w-full h-full m-4 flex flex-col"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-lg font-semibold">
                Biometric Image - ID: {viewingImage}
              </h3>
              <div className="flex items-center gap-2">
                {imageData && (
                  <button
                    onClick={handleDownloadImage}
                    className="compact-button bg-blue-600 text-white flex items-center gap-1"
                  >
                    <Download className="w-4 h-4" />
                    Download
                  </button>
                )}
                <button
                  onClick={handleCloseImage}
                  className="compact-button border border-gray-200 flex items-center gap-1"
                >
                  <X className="w-4 h-4" />
                  Close
                </button>
              </div>
            </div>

            {/* Image Content */}
            <div className="flex-1 flex items-center justify-center bg-gray-50 rounded border border-gray-300">
              {loadingImage ? (
                <div className="flex items-center gap-3">
                  <Loader2 className="w-6 h-6 animate-spin text-blue-600" />
                  <span className="text-gray-600">Loading image...</span>
                </div>
              ) : imageData ? (
                <div className="h-full flex items-center justify-center">
                  {/* Simple image preview for non-fingerprint biometrics */}
                  <img
                    src={imageData}
                    alt={`Biometric ${viewingImage}`}
                    className="max-w-full max-h-full object-contain rounded"
                    onError={(e) => {
                      console.error("Failed to load biometric image:", e);
                      toast.error("Failed to display biometric image");
                    }}
                  />
                </div>
              ) : (
                <div className="text-center">
                  <AlertCircle className="w-12 h-12 text-gray-400 mx-auto mb-2" />
                  <p className="text-gray-600">Failed to load image</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Resize Confirmation Modal */}
      {showResizeModal && pendingUpload && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md w-full mx-4">
            <div className="flex items-center gap-3 mb-4">
              <ImageIcon className="w-6 h-6 text-blue-600" />
              <h3 className="text-lg font-semibold">Image Size Warning</h3>
            </div>

            <div className="space-y-4">
              <p className="text-gray-600">
                The uploaded image is smaller than the recommended dimensions:
              </p>

              <div className="bg-gray-50 rounded-lg p-4">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-600">Current dimensions:</span>
                  <span className="font-medium">
                    {pendingUpload.dimensions.width} ×{" "}
                    {pendingUpload.dimensions.height}px
                  </span>
                </div>
                <div className="flex justify-between text-sm mt-1">
                  <span className="text-gray-600">Recommended minimum:</span>
                  <span className="font-medium text-green-600">
                    709 × 945px
                  </span>
                </div>
              </div>

              <p className="text-gray-600 text-sm">
                Would you like us to resize the image to meet the minimum
                requirements, or upload it as-is with the current dimensions?
              </p>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={() => {
                  setShowResizeModal(false);
                  setPendingUpload(null);
                }}
                className="compact-button border"
                disabled={uploadingPhoto}
              >
                Cancel
              </button>
              <button
                onClick={() => handleResizeConfirmation(false)}
                className="compact-button border"
                disabled={uploadingPhoto}
              >
                {uploadingPhoto ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                    Processing...
                  </>
                ) : (
                  "Keep Original"
                )}
              </button>
              <button
                onClick={() => handleResizeConfirmation(true)}
                className="compact-button bg-blue-600 text-white"
                disabled={uploadingPhoto}
              >
                {uploadingPhoto ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                    Resizing...
                  </>
                ) : (
                  "Resize & Upload"
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Image Cropper Modal */}
      {showCropModal && pendingCrop && (
        <div className="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-4xl max-h-[90vh] w-full mx-4 flex flex-col">
            <div className="flex items-center gap-3 mb-4">
              <CropIcon className="w-6 h-6 text-blue-600" />
              <h3 className="text-lg font-semibold">
                Crop Image to 3:4 Aspect Ratio
              </h3>
            </div>

            <div className="space-y-4 mb-4">
              <div className="text-sm text-gray-600">
                <p>
                  The uploaded image doesn't have the required 3:4 aspect ratio
                  for biometric photographs.
                </p>
                <p>
                  Please crop the image to select the portion you want to use
                  (709×945px).
                </p>
              </div>

              <div className="bg-gray-50 rounded-lg p-3 text-xs">
                <div className="flex justify-between">
                  <span className="text-gray-600">Current aspect ratio:</span>
                  <span className="font-medium">
                    {(
                      pendingCrop.dimensions.width /
                      pendingCrop.dimensions.height
                    ).toFixed(2)}
                    :1
                  </span>
                </div>
                <div className="flex justify-between mt-1">
                  <span className="text-gray-600">Target aspect ratio:</span>
                  <span className="font-medium text-green-600">
                    0.75:1 (3:4)
                  </span>
                </div>
              </div>
            </div>

            {/* Cropper */}
            <div className="flex-1 flex justify-center items-center bg-gray-50 rounded-lg overflow-hidden">
              <ReactCrop
                crop={crop}
                onChange={(c) => setCrop(c)}
                onComplete={(c) => setCompletedCrop(c)}
                aspect={3 / 4}
                keepSelection
                className="max-w-full max-h-full"
              >
                <img
                  src={pendingCrop.imageUrl}
                  alt="Crop preview"
                  className="max-w-full max-h-[60vh] object-contain"
                />
              </ReactCrop>
            </div>

            <div className="flex justify-end gap-3 mt-6">
              <button
                onClick={handleCropCancel}
                className="compact-button border"
                disabled={uploadingPhoto}
              >
                Cancel
              </button>
              <button
                onClick={handleCropConfirmation}
                className="compact-button bg-blue-600 text-white"
                disabled={uploadingPhoto || !completedCrop}
              >
                {uploadingPhoto ? (
                  <>
                    <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                    Cropping...
                  </>
                ) : (
                  <>
                    <CropIcon className="w-3 h-3 mr-1" />
                    Crop & Upload
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
