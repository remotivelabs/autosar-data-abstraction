use crate::communication::{ISignalTriggering, PduTriggering};
use crate::{
    AbstractionElement, ArPackage, AutosarAbstractionError, EcuInstance, IdentifiableAbstractionElement,
    abstraction_element,
};
use autosar_data::{Element, ElementName};

//##################################################################

/// A `Gateway` describes how an ECU forwards PDUs and signals between its physical channels
///
/// Use [`System::create_gateway`](crate::System::create_gateway) to create a new gateway
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Gateway(Element);
abstraction_element!(Gateway, Gateway);
impl IdentifiableAbstractionElement for Gateway {}

impl Gateway {
    pub(crate) fn new(name: &str, package: &ArPackage, ecu: &EcuInstance) -> Result<Self, AutosarAbstractionError> {
        let pkg_elements = package.element().get_or_create_sub_element(ElementName::Elements)?;
        let elem_gateway = pkg_elements.create_named_sub_element(ElementName::Gateway, name)?;
        let gateway = Self(elem_gateway);
        if let Err(error) = gateway.set_ecu(ecu) {
            let _ = pkg_elements.remove_sub_element(gateway.0);
            return Err(error);
        }

        Ok(gateway)
    }

    /// set the ECU that acts as this gateway
    pub fn set_ecu(&self, ecu: &EcuInstance) -> Result<(), AutosarAbstractionError> {
        self.element()
            .get_or_create_sub_element(ElementName::EcuRef)?
            .set_reference_target(ecu.element())?;
        Ok(())
    }

    /// get the ECU that acts as this gateway
    #[must_use]
    pub fn ecu(&self) -> Option<EcuInstance> {
        self.element()
            .get_sub_element(ElementName::EcuRef)
            .and_then(|ecu_ref| ecu_ref.get_reference_target().ok())
            .and_then(|elem| elem.try_into().ok())
    }

    /// create a mapping that forwards the PDU of `source` as the PDU of `target`
    ///
    /// # Example
    ///
    /// ```
    /// # use autosar_data::*;
    /// # use autosar_data_abstraction::*;
    /// # use autosar_data_abstraction::communication::*;
    /// # fn main() -> Result<(), AutosarAbstractionError> {
    /// # let model = AutosarModelAbstraction::create("filename", AutosarVersion::LATEST);
    /// # let package = model.get_or_create_package("/pkg")?;
    /// # let system = package.create_system("System", SystemCategory::SystemExtract)?;
    /// # let ecu = system.create_ecu_instance("Ecu", &package)?;
    /// # let pdu = system.create_isignal_ipdu("Pdu", &package, 8)?;
    /// # let mut triggerings = Vec::new();
    /// # for name in ["Source", "Target"] {
    /// #     let channel = system.create_can_cluster(name, &package, None)?.create_physical_channel(name)?;
    /// #     let frame = system.create_can_frame(&format!("{name}Frame"), &package, 8)?;
    /// #     frame.map_pdu(&pdu, 0, ByteOrder::MostSignificantByteLast, None)?;
    /// #     let frame_triggering = channel.trigger_frame(&frame, 0x100, CanAddressingMode::Standard, CanFrameType::Can20)?;
    /// #     triggerings.push(frame_triggering.pdu_triggerings().next().unwrap());
    /// # }
    /// let gateway = system.create_gateway("Gateway", &package, &ecu)?;
    /// let mapping = gateway.create_i_pdu_mapping(&triggerings[0], &triggerings[1])?;
    /// assert_eq!(mapping.source(), Some(triggerings[0].clone()));
    /// assert_eq!(gateway.i_pdu_mappings().count(), 1);
    /// # Ok(())}
    /// ```
    ///
    /// # Errors
    ///
    /// - [`AutosarAbstractionError::ModelError`] An error occurred in the Autosar model while trying to create the I-PDU-MAPPING
    pub fn create_i_pdu_mapping(
        &self,
        source: &PduTriggering,
        target: &PduTriggering,
    ) -> Result<IPduMapping, AutosarAbstractionError> {
        IPduMapping::new(self, source, target)
    }

    /// iterate over the I-PDU mappings of this gateway
    pub fn i_pdu_mappings(&self) -> impl Iterator<Item = IPduMapping> + Send + use<> {
        self.element()
            .get_sub_element(ElementName::IPduMappings)
            .into_iter()
            .flat_map(|mappings| mappings.sub_elements())
            .filter_map(|elem| IPduMapping::try_from(elem).ok())
    }

    /// create a mapping that forwards the signal of `source` as the signal of `target`
    ///
    /// A gateway forwards a signal on its own where the PDU that carries it cannot be forwarded
    /// whole, between CAN and LIN for example.
    ///
    /// # Example
    ///
    /// ```
    /// # use autosar_data::*;
    /// # use autosar_data_abstraction::*;
    /// # use autosar_data_abstraction::communication::*;
    /// # fn main() -> Result<(), AutosarAbstractionError> {
    /// # let model = AutosarModelAbstraction::create("filename", AutosarVersion::LATEST);
    /// # let package = model.get_or_create_package("/pkg")?;
    /// # let system = package.create_system("System", SystemCategory::SystemExtract)?;
    /// # let ecu = system.create_ecu_instance("Ecu", &package)?;
    /// # let pdu = system.create_isignal_ipdu("Pdu", &package, 8)?;
    /// # let system_signal = package.create_system_signal("SignalValue")?;
    /// # let signal = system.create_isignal("Signal", &package, 8, &system_signal, None)?;
    /// # pdu.map_signal(&signal, 0, ByteOrder::MostSignificantByteLast, None, TransferProperty::Triggered)?;
    /// # let mut triggerings = Vec::new();
    /// # for name in ["Source", "Target"] {
    /// #     let channel = system.create_can_cluster(name, &package, None)?.create_physical_channel(name)?;
    /// #     let frame = system.create_can_frame(&format!("{name}Frame"), &package, 8)?;
    /// #     frame.map_pdu(&pdu, 0, ByteOrder::MostSignificantByteLast, None)?;
    /// #     let frame_triggering = channel.trigger_frame(&frame, 0x100, CanAddressingMode::Standard, CanFrameType::Can20)?;
    /// #     triggerings.push(frame_triggering.pdu_triggerings().next().unwrap().signal_triggerings().next().unwrap());
    /// # }
    /// let gateway = system.create_gateway("Gateway", &package, &ecu)?;
    /// let mapping = gateway.create_i_signal_mapping(&triggerings[0], &triggerings[1])?;
    /// assert_eq!(mapping.source(), Some(triggerings[0].clone()));
    /// assert_eq!(gateway.i_signal_mappings().count(), 1);
    /// # Ok(())}
    /// ```
    ///
    /// Both triggerings trigger a signal, or both a signal group: the gateway forwards a signal as a signal and a
    /// group as a group.
    ///
    /// # Errors
    ///
    /// - [`AutosarAbstractionError::InvalidParameter`] One triggering triggers a signal and the other a signal group
    /// - [`AutosarAbstractionError::ModelError`] An error occurred in the Autosar model while trying to create the I-SIGNAL-MAPPING
    pub fn create_i_signal_mapping(
        &self,
        source: &ISignalTriggering,
        target: &ISignalTriggering,
    ) -> Result<ISignalMapping, AutosarAbstractionError> {
        ISignalMapping::new(self, source, target)
    }

    /// iterate over the I-signal mappings of this gateway
    pub fn i_signal_mappings(&self) -> impl Iterator<Item = ISignalMapping> + Send + use<> {
        self.element()
            .get_sub_element(ElementName::SignalMappings)
            .into_iter()
            .flat_map(|mappings| mappings.sub_elements())
            .filter_map(|elem| ISignalMapping::try_from(elem).ok())
    }
}

//##################################################################

/// An `IPduMapping` forwards the PDU of one PDU triggering as the PDU of another
///
/// Use [`Gateway::create_i_pdu_mapping`] to create a new mapping
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IPduMapping(Element);
abstraction_element!(IPduMapping, IPduMapping);

impl IPduMapping {
    fn new(gateway: &Gateway, source: &PduTriggering, target: &PduTriggering) -> Result<Self, AutosarAbstractionError> {
        let mappings = gateway.element().get_or_create_sub_element(ElementName::IPduMappings)?;
        let mapping = Self(mappings.create_sub_element(ElementName::IPduMapping)?);
        if let Err(error) = mapping.set_source(source).and_then(|()| mapping.set_target(target)) {
            let _ = mapping.remove(false);
            return Err(error);
        }

        Ok(mapping)
    }

    /// get the mapping that holds a SOURCE-I-PDU-REF or TARGET-I-PDU-REF, from the parent of that reference
    pub(crate) fn from_reference_parent(parent: &Element) -> Option<Self> {
        match parent.element_name() {
            ElementName::IPduMapping => Self::try_from(parent.clone()).ok(),
            ElementName::TargetIPdu => Self::try_from(parent.parent().ok()??).ok(),
            _ => None,
        }
    }

    /// remove this `IPduMapping` from the model, and the I-PDU-MAPPINGS of its gateway if no mapping is left
    pub fn remove(self, deep: bool) -> Result<(), AutosarAbstractionError> {
        remove_mapping(self, deep)
    }

    /// set the PDU triggering the gateway receives
    pub fn set_source(&self, source: &PduTriggering) -> Result<(), AutosarAbstractionError> {
        self.element()
            .get_or_create_sub_element(ElementName::SourceIPduRef)?
            .set_reference_target(source.element())?;
        Ok(())
    }

    /// get the PDU triggering the gateway receives
    #[must_use]
    pub fn source(&self) -> Option<PduTriggering> {
        self.element()
            .get_sub_element(ElementName::SourceIPduRef)
            .and_then(|source_ref| source_ref.get_reference_target().ok())
            .and_then(|elem| elem.try_into().ok())
    }

    /// set the PDU triggering the gateway sends
    pub fn set_target(&self, target: &PduTriggering) -> Result<(), AutosarAbstractionError> {
        self.element()
            .get_or_create_sub_element(ElementName::TargetIPdu)?
            .get_or_create_sub_element(ElementName::TargetIPduRef)?
            .set_reference_target(target.element())?;
        Ok(())
    }

    /// get the PDU triggering the gateway sends
    #[must_use]
    pub fn target(&self) -> Option<PduTriggering> {
        self.element()
            .get_sub_element(ElementName::TargetIPdu)
            .and_then(|target| target.get_sub_element(ElementName::TargetIPduRef))
            .and_then(|target_ref| target_ref.get_reference_target().ok())
            .and_then(|elem| elem.try_into().ok())
    }
}

//##################################################################

/// An `ISignalMapping` forwards the signal of one signal triggering as the signal of another
///
/// Use [`Gateway::create_i_signal_mapping`] to create a new mapping
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ISignalMapping(Element);
abstraction_element!(ISignalMapping, ISignalMapping);

impl ISignalMapping {
    fn new(
        gateway: &Gateway,
        source: &ISignalTriggering,
        target: &ISignalTriggering,
    ) -> Result<Self, AutosarAbstractionError> {
        let is_group = |triggering: &ISignalTriggering| {
            triggering
                .element()
                .get_sub_element(ElementName::ISignalGroupRef)
                .is_some()
        };
        if is_group(source) != is_group(target) {
            return Err(AutosarAbstractionError::InvalidParameter(
                "an I-signal mapping maps a signal to a signal, or a signal group to a signal group".to_string(),
            ));
        }
        let mappings = gateway
            .element()
            .get_or_create_sub_element(ElementName::SignalMappings)?;
        let mapping = Self(mappings.create_sub_element(ElementName::ISignalMapping)?);
        if let Err(error) = mapping.set_source(source).and_then(|()| mapping.set_target(target)) {
            let _ = mapping.remove(false);
            return Err(error);
        }
        Ok(mapping)
    }

    /// get the mapping that holds a SOURCE-SIGNAL-REF or TARGET-SIGNAL-REF, from the parent of that reference
    pub(crate) fn from_reference_parent(parent: &Element) -> Option<Self> {
        Self::try_from(parent.clone()).ok()
    }

    /// remove this `ISignalMapping` from the model, and the SIGNAL-MAPPINGS of its gateway if no mapping is left
    pub fn remove(self, deep: bool) -> Result<(), AutosarAbstractionError> {
        remove_mapping(self, deep)
    }

    /// set the signal triggering the gateway receives
    pub fn set_source(&self, source: &ISignalTriggering) -> Result<(), AutosarAbstractionError> {
        self.element()
            .get_or_create_sub_element(ElementName::SourceSignalRef)?
            .set_reference_target(source.element())?;
        Ok(())
    }

    /// get the signal triggering the gateway receives
    #[must_use]
    pub fn source(&self) -> Option<ISignalTriggering> {
        self.element()
            .get_sub_element(ElementName::SourceSignalRef)
            .and_then(|source_ref| source_ref.get_reference_target().ok())
            .and_then(|elem| elem.try_into().ok())
    }

    /// set the signal triggering the gateway sends
    pub fn set_target(&self, target: &ISignalTriggering) -> Result<(), AutosarAbstractionError> {
        self.element()
            .get_or_create_sub_element(ElementName::TargetSignalRef)?
            .set_reference_target(target.element())?;
        Ok(())
    }

    /// get the signal triggering the gateway sends
    #[must_use]
    pub fn target(&self) -> Option<ISignalTriggering> {
        self.element()
            .get_sub_element(ElementName::TargetSignalRef)
            .and_then(|target_ref| target_ref.get_reference_target().ok())
            .and_then(|elem| elem.try_into().ok())
    }
}

//##################################################################

/// remove a mapping from the model, and the mappings container of its gateway if no mapping is left
fn remove_mapping<T: AbstractionElement>(mapping: T, deep: bool) -> Result<(), AutosarAbstractionError> {
    let opt_mappings = mapping.element().parent()?;
    AbstractionElement::remove(mapping, deep)?;
    if let Some(mappings) = opt_mappings
        && mappings.sub_elements().next().is_none()
        && let Some(gateway) = mappings.parent()?
    {
        gateway.remove_sub_element(mappings)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        ArPackage, AutosarModelAbstraction, ByteOrder, System, SystemCategory,
        communication::{AbstractFrame, AbstractFrameTriggering, CanAddressingMode, CanFrameType, TransferProperty},
    };
    use autosar_data::AutosarVersion;

    /// A PDU carrying one signal, triggered on a CAN channel of each name
    fn pdu_triggerings(system: &System, package: &ArPackage, names: &[&str]) -> Vec<PduTriggering> {
        let pdu = system.create_isignal_ipdu("Pdu", package, 8).unwrap();
        let system_signal = package.create_system_signal("SignalValue").unwrap();
        let signal = system
            .create_isignal("Signal", package, 8, &system_signal, None)
            .unwrap();
        pdu.map_signal(
            &signal,
            0,
            ByteOrder::MostSignificantByteLast,
            None,
            TransferProperty::Triggered,
        )
        .unwrap();
        names
            .iter()
            .map(|name| {
                let channel = system
                    .create_can_cluster(name, package, None)
                    .unwrap()
                    .create_physical_channel(name)
                    .unwrap();
                let frame = system.create_can_frame(&format!("{name}Frame"), package, 8).unwrap();
                frame
                    .map_pdu(&pdu, 0, ByteOrder::MostSignificantByteLast, None)
                    .unwrap();
                channel
                    .trigger_frame(&frame, 0x100, CanAddressingMode::Standard, CanFrameType::Can20)
                    .unwrap()
                    .pdu_triggerings()
                    .next()
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn gateway() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let other_ecu = system.create_ecu_instance("OtherEcu", &package).unwrap();
        let triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis"]);

        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        assert_eq!(gateway.ecu(), Some(ecu));
        gateway.set_ecu(&other_ecu).unwrap();
        assert_eq!(gateway.ecu(), Some(other_ecu));
        assert_eq!(system.gateways().collect::<Vec<_>>(), vec![gateway.clone()]);

        assert_eq!(gateway.i_pdu_mappings().count(), 0);
        let mapping = gateway.create_i_pdu_mapping(&triggerings[0], &triggerings[1]).unwrap();
        assert_eq!(mapping.source(), Some(triggerings[0].clone()));
        assert_eq!(mapping.target(), Some(triggerings[1].clone()));
        mapping.set_source(&triggerings[1]).unwrap();
        mapping.set_target(&triggerings[0]).unwrap();
        assert_eq!(mapping.source(), Some(triggerings[1].clone()));
        assert_eq!(mapping.target(), Some(triggerings[0].clone()));
        assert_eq!(gateway.i_pdu_mappings().collect::<Vec<_>>(), vec![mapping]);
    }

    #[test]
    fn signal_mapping() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings: Vec<ISignalTriggering> = pdu_triggerings(&system, &package, &["Body", "Chassis"])
            .iter()
            .map(|triggering| triggering.signal_triggerings().next().unwrap())
            .collect();
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        assert_eq!(gateway.i_signal_mappings().count(), 0);

        let mapping = gateway
            .create_i_signal_mapping(&triggerings[0], &triggerings[1])
            .unwrap();
        assert_eq!(mapping.source(), Some(triggerings[0].clone()));
        assert_eq!(mapping.target(), Some(triggerings[1].clone()));
        mapping.set_source(&triggerings[1]).unwrap();
        mapping.set_target(&triggerings[0]).unwrap();
        assert_eq!(mapping.source(), Some(triggerings[1].clone()));
        assert_eq!(mapping.target(), Some(triggerings[0].clone()));
        assert_eq!(gateway.i_signal_mappings().collect::<Vec<_>>(), vec![mapping.clone()]);

        mapping.remove(false).unwrap();
        assert_eq!(gateway.i_signal_mappings().count(), 0);
        assert!(gateway.element().get_sub_element(ElementName::SignalMappings).is_none());
    }

    #[test]
    fn remove_signal_triggering_that_is_source_and_target() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings: Vec<ISignalTriggering> =
            pdu_triggerings(&system, &package, &["Body", "Chassis", "Powertrain"])
                .iter()
                .map(|triggering| triggering.signal_triggerings().next().unwrap())
                .collect();
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        gateway
            .create_i_signal_mapping(&triggerings[0], &triggerings[0])
            .unwrap();
        // the target reference is registered before the source reference
        let mapping = gateway
            .create_i_signal_mapping(&triggerings[2], &triggerings[1])
            .unwrap();
        mapping.set_source(&triggerings[1]).unwrap();
        let kept = gateway
            .create_i_signal_mapping(&triggerings[2], &triggerings[2])
            .unwrap();

        triggerings[0].clone().remove(false).unwrap();
        triggerings[1].clone().remove(false).unwrap();
        assert_eq!(gateway.i_signal_mappings().collect::<Vec<_>>(), vec![kept]);

        triggerings[2].clone().remove(false).unwrap();
        assert_eq!(gateway.i_signal_mappings().count(), 0);
        assert!(gateway.element().get_sub_element(ElementName::SignalMappings).is_none());

        // the PDU triggerings keep no empty reference to a removed signal triggering
        let model_text = model.files().next().unwrap().serialize().unwrap();
        assert!(
            !model_text.contains("<I-SIGNAL-TRIGGERING-REF-CONDITIONAL/>"),
            "{model_text}"
        );
        assert!(
            !model_text.contains("I-SIGNAL-TRIGGERING-REF-CONDITIONAL>\n"),
            "{model_text}"
        );
    }

    #[test]
    fn remove_pdu_triggering_removes_its_signal_mappings() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let pdu_triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis", "Powertrain"]);
        let triggerings: Vec<ISignalTriggering> = pdu_triggerings
            .iter()
            .map(|triggering| triggering.signal_triggerings().next().unwrap())
            .collect();
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        gateway
            .create_i_signal_mapping(&triggerings[0], &triggerings[1])
            .unwrap();
        let kept = gateway
            .create_i_signal_mapping(&triggerings[1], &triggerings[2])
            .unwrap();

        pdu_triggerings[0].clone().remove(false).unwrap();
        assert_eq!(gateway.i_signal_mappings().collect::<Vec<_>>(), vec![kept]);
    }

    #[test]
    fn signal_mapped_to_signal_group_is_refused() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings: Vec<ISignalTriggering> = pdu_triggerings(&system, &package, &["Body", "Chassis"])
            .iter()
            .map(|triggering| triggering.signal_triggerings().next().unwrap())
            .collect();
        // the target now triggers a signal group instead of a signal
        let target = triggerings[1].element();
        target.remove_sub_element_kind(ElementName::ISignalRef).unwrap();
        let system_group = package.create_system_signal_group("SystemGroup").unwrap();
        let group = system.create_isignal_group("Group", &package, &system_group).unwrap();
        target
            .create_sub_element(ElementName::ISignalGroupRef)
            .unwrap()
            .set_reference_target(group.element())
            .unwrap();
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();

        let result = gateway.create_i_signal_mapping(&triggerings[0], &triggerings[1]);
        assert!(matches!(result, Err(AutosarAbstractionError::InvalidParameter(_))));
        assert_eq!(gateway.i_signal_mappings().count(), 0);
    }

    #[test]
    fn mapping_without_references() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        let mapping = IPduMapping(
            gateway
                .element()
                .create_sub_element(ElementName::IPduMappings)
                .unwrap()
                .create_sub_element(ElementName::IPduMapping)
                .unwrap(),
        );

        assert_eq!(mapping.source(), None);
        assert_eq!(mapping.target(), None);
        assert_eq!(gateway.i_pdu_mappings().count(), 1);
    }

    #[test]
    fn remove_ecu() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        system.create_gateway("Gateway", &package, &ecu).unwrap();

        ecu.remove(false).unwrap();

        assert!(model.get_element_by_path("/package/Gateway").is_none());
        let fibex_elements = system.element().get_sub_element(ElementName::FibexElements).unwrap();
        assert_eq!(fibex_elements.sub_elements().count(), 0);
    }

    #[test]
    fn create_gateway_for_an_ecu_of_another_system() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let other_system = package
            .create_system("OtherSystem", SystemCategory::SystemExtract)
            .unwrap();
        let ecu = other_system.create_ecu_instance("Ecu", &package).unwrap();

        let result = system.create_gateway("Gateway", &package, &ecu);

        assert!(matches!(result, Err(AutosarAbstractionError::InvalidParameter(_))));
        assert!(model.get_element_by_path("/package/Gateway").is_none());
        assert_eq!(system.gateways().count(), 0);
    }

    #[test]
    fn failed_create_leaves_nothing() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let removed_ecu = system.create_ecu_instance("RemovedEcu", &package).unwrap();
        let triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis"]);
        let removed = triggerings[1].clone();
        triggerings[1].clone().remove(false).unwrap();
        removed_ecu.clone().remove(false).unwrap();

        assert!(Gateway::new("RemovedGateway", &package, &removed_ecu).is_err());
        assert!(model.get_element_by_path("/package/RemovedGateway").is_none());

        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        assert!(gateway.create_i_pdu_mapping(&removed, &triggerings[0]).is_err());
        assert!(gateway.create_i_pdu_mapping(&triggerings[0], &removed).is_err());
        assert!(gateway.element().get_sub_element(ElementName::IPduMappings).is_none());
    }

    #[test]
    fn remove_pdu_triggering() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis", "Powertrain"]);
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        gateway.create_i_pdu_mapping(&triggerings[0], &triggerings[1]).unwrap();
        let kept = gateway.create_i_pdu_mapping(&triggerings[1], &triggerings[2]).unwrap();
        gateway.create_i_pdu_mapping(&triggerings[2], &triggerings[0]).unwrap();

        triggerings[0].clone().remove(false).unwrap();
        assert_eq!(gateway.i_pdu_mappings().collect::<Vec<_>>(), vec![kept]);

        triggerings[1].clone().remove(false).unwrap();
        assert_eq!(gateway.i_pdu_mappings().count(), 0);
        assert!(!model.root_element().serialize().contains("I-PDU-MAPPINGS"));
    }

    #[test]
    fn remove_pdu_triggering_that_is_source_and_target() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis", "Powertrain"]);
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        gateway.create_i_pdu_mapping(&triggerings[0], &triggerings[0]).unwrap();
        // the target reference is registered before the source reference
        let mapping = gateway.create_i_pdu_mapping(&triggerings[2], &triggerings[1]).unwrap();
        mapping.set_source(&triggerings[1]).unwrap();
        let kept = gateway.create_i_pdu_mapping(&triggerings[2], &triggerings[2]).unwrap();

        triggerings[0].clone().remove(false).unwrap();
        triggerings[1].clone().remove(false).unwrap();
        assert_eq!(gateway.i_pdu_mappings().collect::<Vec<_>>(), vec![kept]);

        triggerings[2].physical_channel().unwrap().remove(false).unwrap();
        assert_eq!(gateway.i_pdu_mappings().count(), 0);
        assert!(gateway.element().get_sub_element(ElementName::IPduMappings).is_none());
    }

    #[test]
    fn remove_system() {
        let model = AutosarModelAbstraction::create("test", AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package.create_system("System", SystemCategory::SystemExtract).unwrap();
        let ecu = system.create_ecu_instance("Ecu", &package).unwrap();
        let triggerings = pdu_triggerings(&system, &package, &["Body", "Chassis"]);
        let gateway = system.create_gateway("Gateway", &package, &ecu).unwrap();
        gateway.create_i_pdu_mapping(&triggerings[0], &triggerings[1]).unwrap();
        system.create_gateway("OtherGateway", &package, &ecu).unwrap();

        system.remove(true).unwrap();

        assert!(model.get_element_by_path("/package/Gateway").is_none());
        assert!(model.get_element_by_path("/package/OtherGateway").is_none());
        assert!(!model.root_element().serialize().contains("GATEWAY"));
    }
}
