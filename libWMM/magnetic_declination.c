//
//  magnetic_declination.c
//  las2ocd
//
//  Created by Erik Aderstedt on 2018-11-11.
//  Copyright © 2018 Aderstedt Software AB. All rights reserved.
//

#include "magnetic_declination.h"
#include "incbin.h"
#include <stdlib.h>
#include <time.h>

#include "GeomagnetismHeader.h"
#include "EGM9615.h"

INCBIN(WMM, "WMM.COF");

double todays_magnetic_declination(double latitude, double longitude, double height_above_sea_level) {
    MAGtype_MagneticModel * MagneticModels[1], *TimedMagneticModel;
    MAGtype_Ellipsoid Ellip;
    MAGtype_CoordSpherical CoordSpherical;
    MAGtype_CoordGeodetic CoordGeodetic;
    MAGtype_Date UserDate;
    MAGtype_GeoMagneticElements GeoMagneticElements;
    MAGtype_Geoid Geoid;
    char ans[20];
    int NumTerms, nMax = 0;
    /* Memory allocation */

    FILE *MODELFILE = fmemopen((void *restrict)gWMMData, gWMMSize, "rb");
    if (!MAG_robustReadMagModels(MODELFILE, &MagneticModels)) {
        fprintf(stderr, "Could not read magnetic model.\n");
        exit(1);
    }
    
    if(nMax < MagneticModels[0]->nMax) nMax = MagneticModels[0]->nMax;
    NumTerms = ((nMax + 1) * (nMax + 2) / 2);
    TimedMagneticModel = MAG_AllocateModelMemory(NumTerms); /* For storing the time modified WMM Model parameters */
    if(MagneticModels[0] == NULL || TimedMagneticModel == NULL)
    {
        MAG_Error(2);
    }
    MAG_SetDefaults(&Ellip, &Geoid); /* Set default values and constants */
    Geoid.GeoidHeightBuffer = GeoidHeightBuffer;
    Geoid.Geoid_Initialized = 1;

    Geoid.UseGeoid = 1;
    CoordGeodetic.HeightAboveGeoid = height_above_sea_level;
    MAG_ConvertGeoidToEllipsoidHeight(&CoordGeodetic, &Geoid);
    CoordGeodetic.lambda = longitude;
    CoordGeodetic.phi = latitude;
    
    const time_t t_epoch = time(NULL);
    struct tm *t = gmtime(&t_epoch);
    UserDate.Year = t->tm_year + 1900;
    UserDate.Month = t->tm_mon + 1;
    UserDate.Day = t->tm_mday;
    MAG_DateToYear(&UserDate, ans);
    
    MAG_GeodeticToSpherical(Ellip, CoordGeodetic, &CoordSpherical); /*Convert from geodetic to Spherical Equations: 17-18, WMM Technical report*/
    MAG_TimelyModifyMagneticModel(UserDate, MagneticModels[0], TimedMagneticModel); /* Time adjust the coefficients, Equation 19, WMM Technical report */
    MAG_Geomag(Ellip, CoordSpherical, CoordGeodetic, TimedMagneticModel, &GeoMagneticElements); /* Computes the geoMagnetic field elements and their time change*/

    MAG_FreeMagneticModelMemory(TimedMagneticModel);
    MAG_FreeMagneticModelMemory(MagneticModels[0]);

    return GeoMagneticElements.Decl;
}
