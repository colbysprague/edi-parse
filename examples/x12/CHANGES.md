# Cleaning log

Source: https://x12.org/examples, extracted 2026-10-31. Raw copies are in `../edi-examples`.

Every file: one segment per line, ASCII only, element whitespace trimmed (ISA left fixed-width), SE01 recounted.

Not included: 835 Example 07 (Coordination of Benefits), both versions. The site has no data stream for it.

## 835/005010X221/example-02-multiple-claims-single-check.edi
- `SE*182*35681~` -> `SE*183*35681~`: SE01 recounted (182 -> 183)

## 835/005010X221/example-03-claim-specific-negotiated-discount.edi
- `SE*34*35681~` -> `SE*35*35681~`: SE01 recounted (34 -> 35)

## 835/005010X221/example-08-correcting-subscriber_example-8a-claim-submitted-incorrect-subscriber-patient-and-incorrect-id.edi
- `REF*TJ*555555555 ~` -> `REF*TJ*555555555~`: stray whitespace inside element

## 835/008060X322/example-01-dollars-and-data-sent-separately.edi
- `NM1*QC*1*DOE*JANE****MI* ABC123456780~` -> `NM1*QC*1*DOE*JANE****MI*ABC123456780~`: stray whitespace inside element

## 835/008060X322/example-02-multiple-claims-single-check.edi
- `DTM*405*202590827~` -> `DTM*405*20250827~`: DTM02 had 9 digits; every other date in the file is 20250827
- `20250624~` -> `RAS*17*CO*45~`: stray date where an adjustment belongs; 54 billed - 37 paid = 17, matching 005010X221 example 02 (CAS*CO*45*17)
- `SE*182*35681~` -> `SE*191*35681~`: SE01 recounted (182 -> 191)

## 835/008060X322/example-03-claim-specific-negotiated-discount.edi
- `SE*38*35681~` -> `SE*37*35681~`: SE01 recounted (38 -> 37)

## 835/008060X322/example-05-line-service-penalty-tax-or-bonuses-impacting-payment-only_example-5a.edi
- ISA16 `:` -> `>`: file uses > as component separator in SVC01

## 835/008060X322/example-05-line-service-penalty-tax-or-bonuses-impacting-payment-only_example-5b.edi
- ISA16 `:` -> `>`: file uses > as component separator in SVC01

## 835/008060X322/example-05-line-service-penalty-tax-or-bonuses-impacting-payment-only_example-5c.edi
- ISA16 `:` -> `>`: file uses > as component separator in SVC01

## 837/005010X222/example-01-commercial-health-insurance.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-02-encounter.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-03-coordination-benefits-cob_example-3a-claim-billing-provider-payer.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-03-coordination-benefits-cob_example-3b-claim-billing-provider-payer-b.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-03-coordination-benefits-cob_example-3c-claim-payer-payer-b-payer-payer.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-04-medicare-secondary-payer-cob.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)
- `SE*43*000000002~` -> `SE*41*0002~`: SE01 recounted (43 -> 41); SE02 set to ST02 (0002)

## 837/005010X222/example-05-ambulance.edi
- `CLM*051068*766.50**41:B:1*Y*A*Y*Y*P*OA~` -> `CLM*051068*766.50***41:B:1*Y*A*Y*Y*P*OA~`: missing empty CLM03, so every later element sat one position early (facility code in CLM04); inserted it
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-06-chiropractic.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-07-oxygen.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-08-wheelchair.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222A1, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-09-anesthesia.edi
- `CLM*153829140*827**22:B:1*Y*A*Y*Y~` -> `CLM*153829140*827***22:B:1*Y*A*Y*Y~`: missing empty CLM03 (composite sat in CLM04); matches 005010X291 example 07
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-10-drugs_example-10a-drug-administered-physician-office.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222A1, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-10-drugs_example-10b-home-infusion-therapy-pharmacy-adjudicated-ndc-loop-2410.edi
- `PER*IC*Brenda Holly*TE*8019999999~~` -> `PER*IC*Brenda Holly*TE*8019999999~`: doubled segment terminator
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222A1, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-10-drugs_example-10c-home-infusion-therapy-pharmacy-adjudicated-hcpcs-loop-2400-or-ndc.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222A1, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-11-ppo-repriced-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222A1, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X222/example-12-out-network-repriced-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X222, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X223/example-01-institutional_example-1a-institutional-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X223A2, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X223/example-01-institutional_example-1b-two-claims-same-provider.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X223A2, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X223/example-01-institutional_example-1c-ppo-repriced-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X223A2, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X223/example-01-institutional_example-1d-out-network-repriced-claim.edi
- `CLM*W392-49141*14.84**13:A:1*A*Y*Y~` -> `CLM*W392-49141*14.84***13:A:1**A*Y*Y~`: missing empty CLM03 and CLM06 (A landed in CLM06, the provider signature slot); matches 005010X292 example 04
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X223, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X223/example-02-property-and-casualty_example-2a-automobile-accident.edi
- `CLM*67236695521*545**13:A:1*A*Y*Y~` -> `CLM*67236695521*545***13:A:1**A*Y*Y~`: missing empty CLM03 and CLM06; same pattern as example 1d
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X223, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-01-commercial-health-insurance.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-02-coordination-benefits-cob_example-2a-claim-billing-provider-payer.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-02-coordination-benefits-cob_example-2b-claim-billing-provider-payer-b.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-03-predetermination-benefits.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-04-orthodontic-treatment-plan.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-05-sales-tax.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-06-multiple-tooth-numbers.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224A2, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X224/example-07-quantity-greater-1.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X224A2, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-01-commercial-health-insurance.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-02-hmo-plan.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-03-ambulance.edi
- `CLM*051068*766.50**41:B:1*Y*A*Y*Y*P*OA*******08~` -> `CLM*051068*766.50***41:B:1*Y*A*Y*Y*P*OA*******08~`: missing empty CLM03, so every later element sat one position early; inserted it (CLM19 now 08, as in the other 005010X291 examples)
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)
- `SE*46*000017712~` -> `SE*47*000017712~`: SE01 recounted (46 -> 47)

## 837/005010X291/example-04-chiropractic.edi
- removed blank line(s)
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)
- `SE*28*3701~` -> `SE*27*3701~`: SE01 recounted (28 -> 27)

## 837/005010X291/example-05-oxygen.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-06-wheelchair.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-07-anesthesia.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-08-drugs_example-8a-drug-administered-physician-office.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-08-drugs_example-8b-home-infusion-therapy-pharmacy-adjudicated-ndc-loop-2410.edi
- `DTP*472*RD8*20040201-2004020~` -> `DTP*472*RD8*20040201-20040207~`: truncated end date; every other line in the file uses 20040201-20040207
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-08-drugs_example-8c-home-infusion-therapy-pharmacy-adjudicated-hcpcs-loop-2400-or-ndc.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X291/example-09-ppo-repriced-claim.edi
- `CLM*ABC123-RI*28.75***11>B>1*Y*A*Y*Y*P*********08~` -> `CLM*ABC123-RI*28.75***11:B:1*Y*A*Y*Y*P*********08~`: CLM05 used > as component separator; rest of file uses :
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X291, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X292/example-01-institutional-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X292, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X292/example-02-two-claims-same-provider.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X292, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)
- `SE*45*987654~` -> `SE*43*987654~`: SE01 recounted (45 -> 43)

## 837/005010X292/example-03-ppo-repriced-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X292, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X292/example-04-out-network-repriced-claim.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X292, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X298/example-01-adjudicated-claim-submitted-all-payer-claim-data-base-apcd.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X298, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X298/example-02-claim-adjudicated-secondary-payer.edi
- `NM1*IL*1* SMITH*TED****MI*JS00111224444~` -> `NM1*IL*1*SMITH*TED****MI*JS00111224444~`: stray whitespace inside element
- `NM1*IL*1* SMITH*TED****MI*JS00111224444~` -> `NM1*IL*1*SMITH*TED****MI*JS00111224444~`: stray whitespace inside element
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X298, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X299/example-01.edi
- `N4*MIAMI*PL*33413~` -> `N4*MIAMI*FL*33413~`: PL is not a state code; same address is FL everywhere else in the file
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X299, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/005010X299/example-02.edi
- added ISA/GS/GE/IEA envelope (ISA12 00501, GS08 005010X299, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/008060X323/example-anesthesia-laser-eye-surgery.edi
- added ISA/GS/GE/IEA envelope (ISA12 00806, GS08 008060X323, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/008060X323/example-chiropractic-spinal-manipulation-x-ray.edi
- added ISA/GS/GE/IEA envelope (ISA12 00806, GS08 008060X323, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)

## 837/008060X324/example-inpatient-hospital-stay.edi
- `DTP*434*RD8*20250112–20250122~` -> `DTP*434*RD8*20250112-20250122~`: en dash replaced with hyphen
- added ISA/GS/GE/IEA envelope (ISA12 00806, GS08 008060X324, sender/receiver from NM1*41/NM1*40, date/time from BHT, ISA15 T)
