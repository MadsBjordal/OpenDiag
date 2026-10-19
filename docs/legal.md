# Legal and regulatory notes (EU focus)

> This is a research summary to guide design, **not legal advice**. Qualified
> legal counsel must review the risk notice (`apps/ui/src/notice.ts`) and these
> points before any public release.

## Access to diagnostics

* **Regulation (EU) 2018/858** (type approval and market surveillance of
  motor vehicles) requires manufacturers to give independent operators
  unrestricted, standardised access to vehicle OBD information and repair and
  maintenance information (RMI). It gives access rights; it does **not**
  authorise circumventing access controls.
* **Regulation (EU) No 168/2013** covers two- and three-wheel vehicles and
  quadricycles (motorcycles), including OBD and RMI access for those vehicles.
* **Regulation (EC) No 595/2009** covers heavy-duty vehicles (relevant for J1939 later).
* **Security-related RMI (SERMI)**: Commission Delegated Regulation (EU)
  2021/1244 sets up an accreditation scheme for independent operators to
  access security-related information (e.g. immobiliser functions). OpenDiag
  offers no level 4 functions; any future authenticated access must go through
  such legitimate schemes or manufacturer-approved methods.

## Vehicle cybersecurity

* **UN Regulation No 155** (cyber security management) and **No 156**
  (software updates) apply to new vehicle types in the EU. They are a reason
  manufacturers add security gateways. OpenDiag only uses legitimate,
  approved authentication and never bypasses gateways.
* **Cyber Resilience Act, Regulation (EU) 2024/2847**: applies to products
  with digital elements made available in the course of a commercial
  activity. Free open-source software developed outside commercial activity is
  largely outside its scope, but "open-source software stewards" have light
  obligations. Review if the project ever sells builds, support or hardware.

## Emissions and roadworthiness

* Tampering with emissions controls and defeat devices are prohibited
  (Regulation (EC) No 715/2007 and national law). OpenDiag offers no function
  to disable or alter emissions systems.
* Periodic roadworthiness tests (Directive 2014/45/EU) can check OBD status.
  The clear-codes flow warns that readiness monitors reset and the vehicle may
  fail an inspection until they complete.

## Data protection

* **GDPR, Regulation (EU) 2016/679**: a VIN and diagnostic data can be
  personal data when linked to a person (the EDPB's guidelines on connected
  vehicles say so). OpenDiag stores everything locally, needs no account,
  uploads nothing automatically, masks the VIN in exports by default, hides it
  in logs by default, and lets the user delete all vehicle data.
* **Data Act, Regulation (EU) 2023/2854**: gives users rights to data from
  connected products. Relevant context, no obligation on OpenDiag as a local tool.

## Liability and consumer law

* **Product Liability Directive (EU) 2024/2853** covers software. Free and
  open-source software developed or supplied outside a commercial activity is
  excluded. This changes if the project becomes commercial.
* Liability exclusions are only valid as far as national law allows; personal
  injury and statutory consumer rights generally cannot be excluded (e.g. the
  Unfair Contract Terms Directive 93/13/EEC). The notice says this; a
  click-through does not remove statutory liability.
* The app requires acceptance of the risk notice before vehicle-changing
  functions and confirmation that the user owns or may work on the vehicle.

## Software distribution

* GPL-3.0 obligations: provide source, licence text, and installation
  information for user products (see [licensing.md](licensing.md)).
* Third-party licences must be listed in releases (`cargo deny` checks them, see `deny.toml`).
* Google Play distribution is compatible with GPL-3.0. Apple's App Store terms
  are widely considered in conflict with GPL-3.0 (relevant before an iOS app).

## Manufacturer data

Do not commit service manuals, manufacturer databases, ODX/PDX files or
extracted ECU data without a licence that allows redistribution. Definitions
must name their source. See [CONTRIBUTING.md](../CONTRIBUTING.md).
