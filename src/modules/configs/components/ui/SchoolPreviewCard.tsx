//
//  campus-pilot
//  SchoolPreviewCard.tsx - Live Preview Component
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React from 'react';
import { School, Mail, Phone, MapPin, Wifi, WifiOff } from 'lucide-react';
import type { SchoolFormData, LogoPreview } from '../../types';

interface SchoolPreviewCardProps {
  schoolData: SchoolFormData;
  logoPreview: {
    light?: LogoPreview;
    dark?: LogoPreview;
  };
}

export const SchoolPreviewCard: React.FC<SchoolPreviewCardProps> = ({
  schoolData,
  logoPreview
}) => {
  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map(word => word.charAt(0))
      .join('')
      .toUpperCase()
      .substring(0, 2);
  };

  const formatAddress = () => {
    const parts = [
      schoolData.address_line1,
      schoolData.city,
      schoolData.province,
      schoolData.country
    ].filter(Boolean);

    return parts.join(', ') || 'No address provided';
  };

  return (
    <div className="space-y-6">
      {/* Preview Header */}
      <div className="text-center">
        <h3 className="text-lg font-semibold text-gray-900 mb-2">Live Preview</h3>
        <p className="text-sm text-gray-600">See how your school information will appear</p>
        <div className="mt-3 inline-flex items-center gap-2 px-3 py-1 bg-green-50 border border-green-200 rounded-full text-sm text-green-700">
          <WifiOff className="w-3 h-3" />
          Works offline
        </div>
      </div>

      {/* Login Screen Preview */}
      <div className="bg-white rounded-xl shadow-lg border border-gray-200 p-6">
        <div className="text-center space-y-4">
          <div className="w-16 h-16 mx-auto bg-gradient-to-br from-blue-100 to-blue-200 rounded-full flex items-center justify-center">
            {logoPreview.light ? (
              <img
                src={logoPreview.light.url}
                alt="School logo"
                className="w-12 h-12 object-contain rounded-full"
              />
            ) : schoolData.name ? (
              <span className="text-blue-600 font-bold text-lg">
                {getInitials(schoolData.name)}
              </span>
            ) : (
              <School className="w-8 h-8 text-blue-600" />
            )}
          </div>

          <div>
            <h4 className="text-xl font-bold text-gray-900">
              {schoolData.name || 'Your School Name'}
            </h4>
            {schoolData.legal_name && (
              <p className="text-sm text-gray-600 mt-1">
                {schoolData.legal_name}
              </p>
            )}
          </div>

          <div className="bg-gray-50 rounded-lg p-4 space-y-3">
            <h5 className="text-sm font-medium text-gray-700">Login Preview</h5>
            <div className="space-y-2">
              <input
                type="text"
                placeholder="Email"
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm"
                disabled
              />
              <input
                type="password"
                placeholder="Password"
                className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm"
                disabled
              />
              <button className="w-full px-3 py-2 bg-blue-600 text-white rounded-lg text-sm font-medium">
                Sign In
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Receipt Header Preview */}
      <div className="bg-white rounded-xl shadow-lg border border-gray-200 p-6">
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 bg-gradient-to-br from-blue-100 to-blue-200 rounded-full flex items-center justify-center">
                {logoPreview.light ? (
                  <img
                    src={logoPreview.light.url}
                    alt="School logo"
                    className="w-8 h-8 object-contain rounded-full"
                  />
                ) : schoolData.name ? (
                  <span className="text-blue-600 font-bold text-sm">
                    {getInitials(schoolData.name)}
                  </span>
                ) : (
                  <School className="w-6 h-6 text-blue-600" />
                )}
              </div>
              <div>
                <h5 className="font-semibold text-gray-900 text-sm">
                  {schoolData.name || 'Your School Name'}
                </h5>
                <p className="text-xs text-gray-600">Receipt Preview</p>
              </div>
            </div>
          </div>

          <div className="border-t border-gray-200 pt-4 space-y-2">
            {schoolData.email && (
              <div className="flex items-center gap-2 text-xs text-gray-600">
                <Mail className="w-3 h-3" />
                {schoolData.email}
              </div>
            )}
            {schoolData.phone && (
              <div className="flex items-center gap-2 text-xs text-gray-600">
                <Phone className="w-3 h-3" />
                {schoolData.phone}
              </div>
            )}
            <div className="flex items-start gap-2 text-xs text-gray-600">
              <MapPin className="w-3 h-3 mt-0.5 flex-shrink-0" />
              <span>{formatAddress()}</span>
            </div>
            {schoolData.emap_code && (
              <div className="text-xs text-gray-500">
                EMAP: {schoolData.emap_code}
              </div>
            )}
          </div>

          <div className="border-t border-gray-200 pt-3">
            <div className="bg-gray-50 rounded p-3 text-xs text-gray-600">
              <div className="flex justify-between">
                <span>Date:</span>
                <span>26 Sep 2025</span>
              </div>
              <div className="flex justify-between">
                <span>Receipt #:</span>
                <span>REC-001</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Configuration Summary */}
      <div className="bg-white rounded-xl shadow-lg border border-gray-200 p-6">
        <h5 className="font-semibold text-gray-900 mb-4 text-sm">Configuration</h5>
        <div className="space-y-3 text-xs">
          <div className="flex justify-between">
            <span className="text-gray-600">Timezone:</span>
            <span className="text-gray-900">{schoolData.timezone}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Language:</span>
            <span className="text-gray-900">{schoolData.locale}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-600">Logos:</span>
            <span className="text-gray-900">
              {logoPreview.light && logoPreview.dark ? 'Light + Dark' :
               logoPreview.light ? 'Light only' :
               logoPreview.dark ? 'Dark only' : 'None'}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
