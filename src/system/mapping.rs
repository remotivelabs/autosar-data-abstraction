use crate::{
    AbstractionElement, AutosarAbstractionError, EcuInstance, Element, IdentifiableAbstractionElement, System,
    abstraction_element, communication, software_component,
};
use autosar_data::ElementName;
use communication::SystemSignal;
use software_component::{
    AbstractSwComponentType, ClientServerOperation, ComponentPrototype, PortInterface, PortPrototype,
    RootSwCompositionPrototype, SwComponentPrototype, Trigger, VariableDataPrototype,
};

//##################################################################

/// A `SystemMapping` contains mappings in the `System`
///
/// it contains mappings between SWCs and ECUs, as well as between ports and signals
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemMapping(Element);
abstraction_element!(SystemMapping, SystemMapping);
impl IdentifiableAbstractionElement for SystemMapping {}

impl SystemMapping {
    pub(crate) fn new(name: &str, system: &System) -> Result<Self, AutosarAbstractionError> {
        let element = system
            .element()
            .get_or_create_sub_element(ElementName::Mappings)?
            .create_named_sub_element(ElementName::SystemMapping, name)?;

        Ok(Self(element))
    }

    /// get the system that contains this mapping
    pub fn system(&self) -> Result<System, AutosarAbstractionError> {
        let sys_elem = self.element().named_parent()?.unwrap();
        System::try_from(sys_elem)
    }

    /// create a new mapping between a SWC and an ECU
    pub fn map_swc_to_ecu(
        &self,
        name: &str,
        component_prototype: &SwComponentPrototype,
        ecu: &EcuInstance,
    ) -> Result<SwcToEcuMapping, AutosarAbstractionError> {
        let root_composition_prototype =
            self.system()?
                .root_sw_composition()
                .ok_or(AutosarAbstractionError::InvalidParameter(
                    "The root compositon must be set before mapping any swc".to_string(),
                ))?;
        let root_composition_type =
            root_composition_prototype
                .composition()
                .ok_or(AutosarAbstractionError::InvalidParameter(
                    "Incomplete root composition prototype".to_string(),
                ))?;

        let mut context_composition_prototypes = vec![];
        let mut current_composition = component_prototype.parent_composition()?;

        // check if the composition is a child of the root composition; this is needed to ensure that the loop can terminate
        if root_composition_type != current_composition && !root_composition_type.is_parent_of(&current_composition) {
            return Err(AutosarAbstractionError::InvalidParameter(
                "The composition is not a child of the root composition".to_string(),
            ));
        }

        // find all compositions between the root composition and the current composition
        while current_composition != root_composition_type {
            // typical case is that each component is only in one composition, so the for loop should only run once
            for comp_proto in current_composition.instances() {
                // this condition should never fail - it only returns none if comp_proto is the root
                // composition, which we already know is not true
                if let Ok(Some(comp_type)) = comp_proto.parent_composition()
                    && (root_composition_type == comp_type || root_composition_type.is_parent_of(&comp_type))
                {
                    context_composition_prototypes.push(comp_proto.clone());
                    current_composition = comp_type;
                    break;
                }
            }
        }

        // the items were collected in reverse order, so we need to reverse them again
        context_composition_prototypes.reverse();

        SwcToEcuMapping::new(
            name,
            component_prototype,
            &context_composition_prototypes,
            &root_composition_prototype,
            ecu,
            self,
        )
    }

    /// create a new mapping between a sender/receiver port and a signal
    ///
    /// `signal`: the system signal that the port is mapped to
    ///
    /// `data_element`: the data element that is mapped to the signal
    ///
    /// `port_prototype`: the port prototype that contains the data element
    ///
    /// `context_components`: a list of component prototypes from the root up to the component that directly contains the port.
    /// This list may be empty, or it could only contain the final application component prototype containing the port.
    ///
    /// `root_composition_prototype`: the root composition prototype that contains the `swc_prototype`.
    /// Rarely required, but may be needed if multiple root compositions use the same composition/component hierarchy.
    pub fn map_sender_receiver_to_signal<T: Into<PortPrototype> + Clone>(
        &self,
        signal: &SystemSignal,
        data_element: &VariableDataPrototype,
        port_prototype: &T,
        context_components: &[&SwComponentPrototype],
        root_composition_prototype: Option<&RootSwCompositionPrototype>,
    ) -> Result<SenderReceiverToSignalMapping, AutosarAbstractionError> {
        self.map_sender_receiver_to_signal_internal(
            signal,
            data_element,
            &port_prototype.clone().into(),
            context_components,
            root_composition_prototype,
        )
    }

    fn map_sender_receiver_to_signal_internal(
        &self,
        signal: &SystemSignal,
        data_element: &VariableDataPrototype,
        port_prototype: &PortPrototype,
        context_components: &[&SwComponentPrototype],
        root_composition_prototype: Option<&RootSwCompositionPrototype>,
    ) -> Result<SenderReceiverToSignalMapping, AutosarAbstractionError> {
        // sanity checks
        // the port must be a sender/receiver port
        let Some(PortInterface::SenderReceiverInterface(interface)) = port_prototype.port_interface() else {
            return Err(AutosarAbstractionError::InvalidParameter(
                "The port prototype must be a sender/receiver port".to_string(),
            ));
        };

        // the data element must be part of the sender/receiver interface
        if data_element.interface()? != interface {
            return Err(AutosarAbstractionError::InvalidParameter(
                "The data element must be part of the sender/receiver interface".to_string(),
            ));
        }

        // the last context component in the list contains the port prototype
        if let Some(swc_prototype) = context_components.last() {
            let swc_type = port_prototype.component_type()?;
            let swc_prototype_type =
                swc_prototype
                    .component_type()
                    .ok_or(AutosarAbstractionError::InvalidParameter(
                        "invalid SWC prototype: component type ref is missing".to_string(),
                    ))?;
            if swc_type != swc_prototype_type {
                return Err(AutosarAbstractionError::InvalidParameter(
                    "The port must be part of the component prototype".to_string(),
                ));
            }
        }

        // create the mapping
        let data_mappings = self.element().get_or_create_sub_element(ElementName::DataMappings)?;

        SenderReceiverToSignalMapping::new(
            &data_mappings,
            signal,
            data_element,
            port_prototype,
            context_components,
            root_composition_prototype,
        )
    }
}

//#########################################################

/// A `SwcToEcuMapping` contains a mapping between a `SwComponentPrototype` and an `EcuInstance`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SwcToEcuMapping(Element);
abstraction_element!(SwcToEcuMapping, SwcToEcuMapping);
impl IdentifiableAbstractionElement for SwcToEcuMapping {}

impl SwcToEcuMapping {
    pub(crate) fn new(
        name: &str,
        component_prototype: &SwComponentPrototype,
        context_composition_prototypes: &[ComponentPrototype],
        root_composition_prototype: &RootSwCompositionPrototype,
        ecu: &EcuInstance,
        mapping: &SystemMapping,
    ) -> Result<Self, AutosarAbstractionError> {
        let sw_mappings_elem = mapping.element().get_or_create_sub_element(ElementName::SwMappings)?;
        let swc_to_ecu_mapping = sw_mappings_elem.create_named_sub_element(ElementName::SwcToEcuMapping, name)?;

        let iref = swc_to_ecu_mapping
            .create_sub_element(ElementName::ComponentIrefs)?
            .create_sub_element(ElementName::ComponentIref)?;

        // create the references to root composition and context compositions
        iref.create_sub_element(ElementName::ContextCompositionRef)?
            .set_reference_target(root_composition_prototype.element())?;
        for context_comp in context_composition_prototypes {
            iref.create_sub_element(ElementName::ContextComponentRef)?
                .set_reference_target(context_comp.element())?;
        }
        // create the reference to the target component prototype
        iref.create_sub_element(ElementName::TargetComponentRef)?
            .set_reference_target(component_prototype.element())?;

        swc_to_ecu_mapping
            .create_sub_element(ElementName::EcuInstanceRef)?
            .set_reference_target(ecu.element())?;

        Ok(Self(swc_to_ecu_mapping))
    }

    /// get the component prototype that is mapped here
    #[must_use]
    pub fn target_component(&self) -> Option<SwComponentPrototype> {
        self.element()
            .get_sub_element(ElementName::ComponentIrefs)
            .and_then(|irefs| irefs.get_sub_element(ElementName::ComponentIref))
            .and_then(|iref| iref.get_sub_element(ElementName::TargetComponentRef))
            .and_then(|target| target.get_reference_target().ok())
            .and_then(|target| SwComponentPrototype::try_from(target).ok())
    }

    /// get the ECU instance which is the target of this mapping
    #[must_use]
    pub fn ecu_instance(&self) -> Option<EcuInstance> {
        self.element()
            .get_sub_element(ElementName::EcuInstanceRef)
            .and_then(|r| r.get_reference_target().ok())
            .and_then(|target| EcuInstance::try_from(target).ok())
    }
}

//#########################################################

/// A `SenderReceiverToSignalMapping` contains a mapping between a sender/receiver port and a system signal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SenderReceiverToSignalMapping(Element);
abstraction_element!(SenderReceiverToSignalMapping, SenderReceiverToSignalMapping);

impl SenderReceiverToSignalMapping {
    pub(crate) fn new(
        parent: &Element,
        signal: &SystemSignal,
        data_element: &VariableDataPrototype,
        port_prototype: &PortPrototype,
        context_components: &[&SwComponentPrototype],
        root_composition_prototype: Option<&RootSwCompositionPrototype>,
    ) -> Result<Self, AutosarAbstractionError> {
        let sr_mapping = parent.create_sub_element(ElementName::SenderReceiverToSignalMapping)?;
        let iref = sr_mapping.create_sub_element(ElementName::DataElementIref)?;
        iref.create_sub_element(ElementName::ContextPortRef)?
            .set_reference_target(port_prototype.element())?;
        iref.create_sub_element(ElementName::TargetDataPrototypeRef)?
            .set_reference_target(data_element.element())?;

        // the list of context components is ordered, with the root composition prototype at the beginning
        for comp_proto in context_components {
            iref.create_sub_element(ElementName::ContextComponentRef)?
                .set_reference_target(comp_proto.element())?;
        }

        if let Some(root_composition_prototype) = root_composition_prototype {
            iref.create_sub_element(ElementName::ContextCompositionRef)?
                .set_reference_target(root_composition_prototype.element())?;
        }

        sr_mapping
            .create_sub_element(ElementName::SystemSignalRef)?
            .set_reference_target(signal.element())?;

        Ok(Self(sr_mapping))
    }

    /// Get the system signal that is the target of this mapping
    #[must_use]
    pub fn system_signal(&self) -> Option<SystemSignal> {
        let element = self
            .element()
            .get_sub_element(ElementName::SystemSignalRef)
            .and_then(|r| r.get_reference_target().ok())?;
        SystemSignal::try_from(element).ok()
    }

    /// Get the data element that is mapped to the signal
    #[must_use]
    pub fn data_element(&self) -> Option<VariableDataPrototype> {
        let element = self
            .element()
            .get_sub_element(ElementName::DataElementIref)
            .and_then(|iref| iref.get_sub_element(ElementName::TargetDataPrototypeRef))
            .and_then(|r| r.get_reference_target().ok())?;
        VariableDataPrototype::try_from(element).ok()
    }
}

//#########################################################

/// A `ClientServerToSignalMapping` maps a client/server operation to the system signals that carry its call and its return
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientServerToSignalMapping(Element);
abstraction_element!(ClientServerToSignalMapping, ClientServerToSignalMapping);

impl ClientServerToSignalMapping {
    /// Get the operation that is mapped
    #[must_use]
    pub fn operation(&self) -> Option<ClientServerOperation> {
        let element = self
            .element()
            .get_sub_element(ElementName::ClientServerOperationIref)
            .and_then(|iref| iref.get_sub_element(ElementName::TargetOperationRef))
            .and_then(|r| r.get_reference_target().ok())?;
        ClientServerOperation::try_from(element).ok()
    }

    /// Get the system signal that carries the call, if there is one
    #[must_use]
    pub fn call_signal(&self) -> Option<SystemSignal> {
        self.referenced_signal(ElementName::CallSignalRef)
    }

    /// Get the system signal that carries the return, if there is one
    #[must_use]
    pub fn return_signal(&self) -> Option<SystemSignal> {
        self.referenced_signal(ElementName::ReturnSignalRef)
    }

    fn referenced_signal(&self, reference: ElementName) -> Option<SystemSignal> {
        let element = self.element().get_sub_element(reference)?.get_reference_target().ok()?;
        SystemSignal::try_from(element).ok()
    }
}

//#########################################################

/// A `TriggerToSignalMapping` maps a trigger to the system signal that carries it
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerToSignalMapping(Element);
abstraction_element!(TriggerToSignalMapping, TriggerToSignalMapping);

impl TriggerToSignalMapping {
    /// Get the system signal that is the target of this mapping
    #[must_use]
    pub fn system_signal(&self) -> Option<SystemSignal> {
        let element = self
            .element()
            .get_sub_element(ElementName::SystemSignalRef)?
            .get_reference_target()
            .ok()?;
        SystemSignal::try_from(element).ok()
    }

    /// Get the trigger that is mapped
    #[must_use]
    pub fn trigger(&self) -> Option<Trigger> {
        let element = self
            .element()
            .get_sub_element(ElementName::TriggerIref)
            .and_then(|iref| iref.get_sub_element(ElementName::TargetTriggerRef))
            .and_then(|r| r.get_reference_target().ok())?;
        Trigger::try_from(element).ok()
    }
}

//#########################################################

/// A mapping of a data element, an operation or a trigger to a system signal
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignalMapping {
    /// a data element of a sender/receiver interface
    SenderReceiver(SenderReceiverToSignalMapping),
    /// an operation of a client/server interface, through its call signal or its return signal
    ClientServer(ClientServerToSignalMapping),
    /// a trigger of a trigger interface
    Trigger(TriggerToSignalMapping),
}

impl TryFrom<Element> for SignalMapping {
    type Error = AutosarAbstractionError;

    fn try_from(element: Element) -> Result<Self, Self::Error> {
        match element.element_name() {
            ElementName::SenderReceiverToSignalMapping => {
                Ok(Self::SenderReceiver(SenderReceiverToSignalMapping::try_from(element)?))
            }
            ElementName::ClientServerToSignalMapping => {
                Ok(Self::ClientServer(ClientServerToSignalMapping::try_from(element)?))
            }
            ElementName::TriggerToSignalMapping => Ok(Self::Trigger(TriggerToSignalMapping::try_from(element)?)),
            _ => Err(AutosarAbstractionError::ConversionError {
                element,
                dest: "SignalMapping".to_string(),
            }),
        }
    }
}

//#########################################################

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        AutosarModelAbstraction, SystemCategory,
        datatype::{ApplicationPrimitiveCategory, ApplicationPrimitiveDataType},
    };

    #[test]
    fn mappings() {
        let model = AutosarModelAbstraction::create("filename", autosar_data::AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package
            .create_system("test_system", SystemCategory::EcuExtract)
            .unwrap();
        let mapping = system.get_or_create_mapping("test_mapping").unwrap();

        let ecu = system.create_ecu_instance("test_ecu", &package).unwrap();
        let root_composition_type = package.create_composition_sw_component_type("test_swc").unwrap();
        let _root_composition = system
            .set_root_sw_composition("test_root_composition", &root_composition_type)
            .unwrap();

        let ecu_composition_type = package
            .create_composition_sw_component_type("Ecu_A_Composition")
            .unwrap();
        let ecu_composition_prototype = root_composition_type
            .create_component("Ecu_A_Composition_Prototype", &ecu_composition_type)
            .unwrap();

        // map ecu_composition_prototype to the ecu
        let swc_to_ecu = mapping
            .map_swc_to_ecu("test_swc_to_ecu", &ecu_composition_prototype, &ecu)
            .unwrap();

        assert_eq!(swc_to_ecu.target_component().unwrap(), ecu_composition_prototype);
        assert_eq!(swc_to_ecu.ecu_instance().unwrap(), ecu);

        // map a signal to a port
        let sys_signal = package.create_system_signal("test_signal").unwrap();

        let sender_receiver_interface = package
            .create_sender_receiver_interface("SenderReceiverInterface")
            .unwrap();
        let data_type = ApplicationPrimitiveDataType::new(
            "Primitive",
            &package,
            ApplicationPrimitiveCategory::Value,
            None,
            None,
            None,
        )
        .unwrap();
        let data_element = sender_receiver_interface
            .create_data_element("element", &data_type)
            .unwrap();
        let sr_port = ecu_composition_type
            .create_r_port("test_port", &sender_receiver_interface)
            .unwrap();

        mapping
            .map_sender_receiver_to_signal(&sys_signal, &data_element, &sr_port, &[], None)
            .unwrap();
    }

    #[test]
    fn signal_mappings() {
        let model = AutosarModelAbstraction::create("filename", autosar_data::AutosarVersion::LATEST);
        let package = model.get_or_create_package("/package").unwrap();
        let system = package
            .create_system("test_system", SystemCategory::EcuExtract)
            .unwrap();
        let mapping = system.get_or_create_mapping("test_mapping").unwrap();
        let data_mappings = mapping
            .element()
            .get_or_create_sub_element(ElementName::DataMappings)
            .unwrap();

        // an operation, carried by a call signal and a return signal
        let call_signal = package.create_system_signal("call").unwrap();
        let return_signal = package.create_system_signal("return").unwrap();
        let interface = package.create_client_server_interface("interface").unwrap();
        let operation = interface.create_operation("operation").unwrap();
        let cs_mapping = data_mappings
            .create_sub_element(ElementName::ClientServerToSignalMapping)
            .unwrap();
        cs_mapping
            .create_sub_element(ElementName::CallSignalRef)
            .unwrap()
            .set_reference_target(call_signal.element())
            .unwrap();
        cs_mapping
            .create_sub_element(ElementName::ClientServerOperationIref)
            .unwrap()
            .create_sub_element(ElementName::TargetOperationRef)
            .unwrap()
            .set_reference_target(operation.element())
            .unwrap();
        cs_mapping
            .create_sub_element(ElementName::ReturnSignalRef)
            .unwrap()
            .set_reference_target(return_signal.element())
            .unwrap();

        // a trigger, carried by a signal
        let trigger_signal = package.create_system_signal("trigger").unwrap();
        let trigger_interface = package.create_trigger_interface("triggers").unwrap();
        let trigger = trigger_interface.create_trigger("trigger").unwrap();
        assert_eq!(trigger_interface.triggers().collect::<Vec<_>>(), vec![trigger.clone()]);
        let trigger_mapping = data_mappings
            .create_sub_element(ElementName::TriggerToSignalMapping)
            .unwrap();
        trigger_mapping
            .create_sub_element(ElementName::SystemSignalRef)
            .unwrap()
            .set_reference_target(trigger_signal.element())
            .unwrap();
        trigger_mapping
            .create_sub_element(ElementName::TriggerIref)
            .unwrap()
            .create_sub_element(ElementName::TargetTriggerRef)
            .unwrap()
            .set_reference_target(trigger.element())
            .unwrap();

        let call_mappings = call_signal.mappings();
        let [SignalMapping::ClientServer(cs)] = call_mappings.as_slice() else {
            panic!("the call signal is mapped once, to the operation");
        };
        assert_eq!(cs.operation(), Some(operation));
        assert_eq!(cs.call_signal(), Some(call_signal.clone()));
        assert_eq!(cs.return_signal(), Some(return_signal.clone()));
        assert_eq!(return_signal.mappings(), vec![SignalMapping::ClientServer(cs.clone())]);

        let trigger_mappings = trigger_signal.mappings();
        let [SignalMapping::Trigger(trigger_to_signal)] = trigger_mappings.as_slice() else {
            panic!("the trigger signal is mapped once, to the trigger");
        };
        assert_eq!(trigger_to_signal.trigger(), Some(trigger));
        assert_eq!(trigger_to_signal.system_signal(), Some(trigger_signal.clone()));
        assert!(package.create_system_signal("unmapped").unwrap().mappings().is_empty());
    }
}
